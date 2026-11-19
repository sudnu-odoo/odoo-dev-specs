//! The Odoo runtime against fake `git`, `python`, `psql` and `dropdb` programs:
//! no network, PostgreSQL or real Odoo needed. The fake Odoo "server" is Python's
//! static web server on the requested port.

use std::fs;
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use odoo_runtime::runtime::{Runtime, Tools};
use tempfile::TempDir;

/// Tests write scripts and then run them; one at a time avoids "Text file busy".
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

struct Env {
    rt: Runtime,
    dir: TempDir,
    _serial: MutexGuard<'static, ()>,
}

impl Env {
    /// `existing`: databases PostgreSQL already has. `fail_create`: odoo-bin fails to init.
    fn new(existing: &[&str], fail_create: bool) -> Env {
        let serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let (log, dbs, bin) = (d.join("calls.log"), d.join("databases.txt"), d.join("bin"));
        fs::create_dir_all(&bin).unwrap();
        fs::write(
            &dbs,
            existing
                .iter()
                .map(|n| format!("{n}\n"))
                .collect::<String>(),
        )
        .unwrap();
        let log = log.display();
        let dbs = dbs.display();

        // git, as the runtime uses it: a shared bare repository per edition, a shallow
        // fetch per version (refused while `deny` exists), a worktree per version
        // (a tiny "Odoo" checkout), the branch list, and updates.
        script(
            &bin.join("git"),
            &format!(
                r#"echo "git $*" >> '{log}'
[ -n "$GIT_SSH_COMMAND" ] && echo "ssh: $GIT_SSH_COMMAND" >> '{d}/ssh.log'
case "$*" in
  *"init --bare"*) for last; do :; done; mkdir -p "$last"; exit 0;;
  *ls-remote*)
    [ -f '{d}/deny' ] && {{ echo 'git@github.com: Permission denied (publickey).' >&2; exit 128; }}
    printf 'a\trefs/heads/16.0\na\trefs/heads/17.0\na\trefs/heads/saas-17.4\na\trefs/heads/18.0\na\trefs/heads/saas-18.4\na\trefs/heads/19.0\na\trefs/heads/saas-19.1\na\trefs/heads/saas-19.2\na\trefs/heads/master\na\trefs/heads/19.0-fix-something\n'
    exit 0;;
  *" fetch "*)
    [ -f '{d}/deny' ] && {{ echo 'git@github.com: Permission denied (publickey).' >&2; echo 'fatal: Could not read from remote repository.' >&2; exit 128; }}
    printf 'Receiving objects:  50%% (5/10)\r' >&2
    printf 'Receiving objects: 100%% (10/10), done.\n' >&2
    printf 'Resolving deltas: 100%% (3/3), done.\n' >&2
    exit 0;;
  *"worktree add"*)
    prev=""; target=""; for a in "$@"; do [ "$prev" = "--force" ] && target="$a"; prev="$a"; done
    mkdir -p "$target/addons" "$target/odoo/addons" "$target/web_enterprise"
    echo '#!/bin/sh' > "$target/odoo-bin"
    echo 'version_info = (19, 0, 0, FINAL, 0)' > "$target/odoo/release.py"
    printf 'psycopg2==2.9.9\npython-ldap==3.4.4\nlxml==5.2.1\n' > "$target/requirements.txt"
    echo ".git" > "$target/.git"
    exit 0;;
  *"worktree remove"*)
    prev=""; for a in "$@"; do [ "$prev" = "--force" ] && rm -rf "$a"; prev="$a"; done; exit 0;;
  *"worktree prune"*) exit 0;;
  *"reset --hard"*) exit 0;;
esac
exit 0"#,
                d = d.display()
            ),
        );
        // python: -m venv DIR | -m pip install -r FILE | <odoo-bin> args...
        let fail = if fail_create { "exit 1" } else { ":" };
        script(
            &bin.join("python3"),
            &format!(
                r#"echo "env PYTHONPATH=$PYTHONPATH PYTHONHOME=$PYTHONHOME NOUSERSITE=$PYTHONNOUSERSITE" >> '{d}/env.log'
if [ "$1" = "-c" ]; then
  echo "python -c $2" >> '{log}'
  [ -f '{d}/broken-imports' ] && {{ echo "ModuleNotFoundError: No module named 'babel'" >&2; exit 1; }}
  exit 0
fi
if [ "$1" = "-m" ] && [ "$2" = "venv" ]; then
  echo "python -m venv" >> '{log}'
  mkdir -p "$3/bin"; ln -sf "$0" "$3/bin/python"; exit 0
fi
if [ "$1" = "-m" ] && [ "$2" = "pip" ]; then
  echo "pip $*" >> '{log}'
  for last; do :; done
  cp "$last" '{d}/pip-requirements.txt'; exit 0
fi
echo "odoo $*" >> '{log}'
db=""; port=""; init=""; prev=""
for a in "$@"; do
  [ "$prev" = "-d" ] && db="$a"; [ "$prev" = "--http-port" ] && port="$a"
  [ "$a" = "--stop-after-init" ] && init=1; prev="$a"
done
if [ -n "$init" ]; then
  # Like odoo-bin: the database exists as soon as it starts, ready only at the end.
  grep -qx "$db" '{dbs}' || echo "$db" >> '{dbs}'
  sleep 0.3
  {fail}
  touch '{d}/ready-'"$db"
  # Remember what is installed, for the installed-apps query.
  prev=""; for a in "$@"; do [ "$prev" = "-i" ] && echo "$a" | tr ',' '\n' >> '{d}/modules-'"$db"; prev="$a"; done
  exit 0
fi
exec /usr/bin/python3 -m http.server "$port" --bind 127.0.0.1"#,
                d = d.display()
            ),
        );
        // psql -d DB -At -c SQL: copies a database, lists installed apps, or lists databases.
        script(
            &bin.join("psql"),
            &format!(
                r#"echo "psql" >> '{log}'
db="$2"; for last; do :; done
case "$last" in
  *"CREATE DATABASE"*)
    echo "psql $last" >> '{log}'
    new=$(echo "$last" | cut -d'"' -f2); old=$(echo "$last" | cut -d'"' -f4)
    echo "$new" >> '{dbs}'; cp '{d}/modules-'"$old" '{d}/modules-'"$new" 2>/dev/null; exit 0;;
  *ir_module_module*) cat '{d}/modules-'"$db" 2>/dev/null; exit 0;;
esac
cat '{dbs}'"#,
                d = d.display()
            ),
        );
        script(
            &bin.join("dropdb"),
            &format!(
                r#"echo "dropdb $*" >> '{log}'
for last; do :; done
grep -vx "$last" '{dbs}' > '{dbs}.tmp'; mv '{dbs}.tmp' '{dbs}'"#
            ),
        );

        // ssh-keygen -t ed25519 -N "" -C ... -f KEY: a fake key pair.
        script(
            &bin.join("ssh-keygen"),
            &format!(
                r#"echo "ssh-keygen $*" >> '{log}'
for last; do :; done
echo PRIVATE > "$last"; echo "ssh-ed25519 AAAAfake odoo-dev-specs" > "$last.pub""#
            ),
        );
        let tools = Tools {
            git: bin.join("git"),
            python: bin.join("python3"),
            psql: bin.join("psql"),
            dropdb: bin.join("dropdb"),
            ssh_keygen: bin.join("ssh-keygen"),
        };
        let rt = Runtime::new(d.join("runtime"), tools)
            .with_odoo_git_url("https://example.invalid/odoo.git");
        Env {
            rt,
            dir,
            _serial: serial,
        }
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join("calls.log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    fn count(&self, prefix: &str) -> usize {
        self.calls()
            .iter()
            .filter(|c| c.starts_with(prefix))
            .count()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.rt.stop_all();
    }
}

#[test]
fn first_use_of_a_version_gets_the_source_and_a_virtualenv_once() {
    let env = Env::new(&[], false);
    let db = env
        .rt
        .ensure_database("19.0", &["sale_management".into()], true)
        .unwrap();
    assert_eq!(db, "specs_19_0_sale_management_demo");
    let create = env
        .calls()
        .into_iter()
        .find(|c| c.starts_with("odoo "))
        .unwrap();
    // Odoo 19 only loads demo data when asked.
    assert!(create.contains("--with-demo"), "{create}");
    assert!(!create.contains("--without-demo"), "{create}");

    let fetch = env
        .calls()
        .into_iter()
        .find(|c| c.contains(" fetch "))
        .unwrap();
    assert!(
        fetch.contains(
            "--depth 1 https://example.invalid/odoo.git +refs/heads/19.0:refs/heads/19.0"
        ),
        "{fetch}"
    );
    assert!(env.rt.source_dir("19.0").join("odoo-bin").is_file());
    assert_eq!(env.count("python -m venv"), 1);
    // Requirements are installed adjusted for pre-built wheels.
    let installed = fs::read_to_string(env.dir.path().join("pip-requirements.txt")).unwrap();
    assert_eq!(installed, "psycopg2-binary==2.9.9\nlxml==5.2.1\n");

    // A second database of the same version reuses source and virtualenv.
    env.rt
        .ensure_database("19.0", &["stock".into()], true)
        .unwrap();
    assert_eq!(
        env.count("git"),
        4,
        "init, fetch, worktree prune, worktree add: {:?}",
        env.calls()
    );
    assert_eq!(env.count("python -m venv"), 1);
}

#[test]
fn creates_the_database_with_odoo_bin() {
    let env = Env::new(&[], false);
    env.rt
        .ensure_database("19.0", &["sale".into(), "stock".into()], false)
        .unwrap();
    let create = env
        .calls()
        .into_iter()
        .find(|c| c.starts_with("odoo "))
        .unwrap();
    let src = env.rt.source_dir("19.0");
    assert!(
        create.contains(&format!("{}/odoo-bin", src.display())),
        "{create}"
    );
    assert!(
        create.contains("-d specs_19_0_sale_stock_nodemo"),
        "{create}"
    );
    assert!(create.contains("-i sale,stock"), "{create}");
    assert!(create.contains("--without-demo=all"), "{create}");
    assert!(create.contains("--stop-after-init"), "{create}");
    assert!(
        create.contains(&format!("--addons-path {}/addons", src.display())),
        "{create}"
    );
    assert!(create.contains("--data-dir"), "{create}");
}

#[test]
fn an_existing_database_is_reused_without_setup() {
    let env = Env::new(&["specs_19_0_base_demo", "someone_elses_db"], false);
    assert_eq!(
        env.rt.databases().unwrap(),
        ["specs_19_0_base_demo"],
        "only our databases"
    );
    env.rt.ensure_database("19.0", &[], true).unwrap();
    assert_eq!(env.calls(), ["psql", "psql"]);
}

#[test]
fn a_failed_creation_is_cleaned_up() {
    let env = Env::new(&[], true);
    let err = env
        .rt
        .ensure_database("19.0", &["sale".into()], true)
        .unwrap_err();
    assert!(format!("{err:#}").contains("Could not create"), "{err:#}");
    assert_eq!(
        env.calls().last().unwrap(),
        "dropdb --if-exists specs_19_0_sale_demo"
    );
    assert!(env.rt.databases().unwrap().is_empty());
}

#[test]
fn delete_removes_the_database_and_its_files() {
    let env = Env::new(&["specs_19_0_base"], false);
    let filestore = env.rt.data_dir().join("filestore").join("specs_19_0_base");
    fs::create_dir_all(&filestore).unwrap();
    env.rt.delete_database("specs_19_0_base").unwrap();
    assert!(env.rt.databases().unwrap().is_empty());
    assert!(!filestore.exists());
    // Never touches databases that are not ours.
    assert!(env.rt.delete_database("someone_elses_db").is_err());
}

#[test]
fn runs_reuses_and_stops_a_server() {
    let env = Env::new(&["specs_19_0_base"], false);
    let first = env.rt.ensure_running("19.0", "specs_19_0_base").unwrap();
    assert!(TcpStream::connect(("127.0.0.1", first.port)).is_ok());
    let run = env
        .calls()
        .into_iter()
        .find(|c| c.contains("--http-port"))
        .unwrap();
    assert!(
        run.contains("-d specs_19_0_base") && !run.contains("--stop-after-init"),
        "{run}"
    );

    let again = env.rt.ensure_running("19.0", "specs_19_0_base").unwrap();
    assert_eq!(again, first);
    assert_eq!(
        env.calls()
            .iter()
            .filter(|c| c.contains("--http-port"))
            .count(),
        1
    );

    env.rt.stop_all().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(TcpStream::connect(("127.0.0.1", first.port)).is_err());
}

#[test]
fn rejects_versions_that_are_not_odoo_branches() {
    let env = Env::new(&[], false);
    for bad in ["", "19", "--upload-pack=x", "19.0; rm -rf /", "../19.0"] {
        assert!(
            env.rt.ensure_database(bad, &[], true).is_err(),
            "accepted {bad:?}"
        );
    }
    assert_eq!(env.count("git"), 0);
}

#[test]
fn an_enterprise_folder_is_added_to_the_addons_path() {
    let env = Env::new(&[], false);
    let enterprise = env.dir.path().join("enterprise-19");
    fs::create_dir_all(&enterprise).unwrap();
    let rt = Runtime::new(env.dir.path().join("runtime"), env.rt.tools().clone())
        .with_odoo_git_url("https://example.invalid/odoo.git")
        .with_enterprise("19.0", &enterprise);
    let db = rt.ensure_database("19.0", &[], true).unwrap();
    // Enterprise installs more (e.g. web_enterprise): never mixed up with community.
    assert_eq!(db, "specs_19_0_base_ee_demo");
    let create = env
        .calls()
        .into_iter()
        .find(|c| c.starts_with("odoo "))
        .unwrap();
    let src = rt.source_dir("19.0");
    assert!(
        create.contains(&format!(
            "--addons-path {},{}/addons",
            enterprise.display(),
            src.display()
        )),
        "{create}"
    );
}

#[test]
fn an_enterprise_folder_next_to_the_source_is_used_automatically() {
    let env = Env::new(&[], false);
    env.rt.ensure_source("19.0").unwrap();
    let enterprise = env
        .rt
        .source_dir("19.0")
        .parent()
        .unwrap()
        .join("enterprise");
    fs::create_dir_all(&enterprise).unwrap();
    let db = env
        .rt
        .ensure_database("19.0", &["sale".into()], true)
        .unwrap();
    assert_eq!(db, "specs_19_0_sale_ee_demo");
    let create = env
        .calls()
        .into_iter()
        .find(|c| c.starts_with("odoo "))
        .unwrap();
    assert!(
        create.contains(&enterprise.display().to_string()),
        "{create}"
    );
    // The assistant reads enterprise code too.
    assert!(
        env.rt.source_dirs().contains(&enterprise),
        "{:?}",
        env.rt.source_dirs()
    );
}

#[test]
fn source_dirs_lists_the_downloaded_versions_for_the_assistant() {
    let env = Env::new(&[], false);
    assert!(env.rt.source_dirs().is_empty());
    env.rt.ensure_database("19.0", &[], true).unwrap();
    assert_eq!(env.rt.source_dirs(), [env.rt.source_dir("19.0")]);
}

#[test]
fn two_callers_creating_the_same_database_wait_for_one_complete_creation() {
    let env = Env::new(&[], false);
    let ready = env.dir.path().join("ready-specs_19_0_sale_demo");
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| {
                let db = env
                    .rt
                    .ensure_database("19.0", &["sale".into()], true)
                    .unwrap();
                assert_eq!(db, "specs_19_0_sale_demo");
                // Never handed out half-made: the creation has finished.
                assert!(ready.exists(), "got the database before it was ready");
            });
        }
    });
    assert_eq!(env.count("odoo "), 1, "created once: {:?}", env.calls());
}

#[test]
fn two_callers_starting_the_same_server_share_one() {
    let env = Env::new(&["specs_19_0_base_demo"], false);
    let ports: Vec<u16> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                s.spawn(|| {
                    env.rt
                        .ensure_running("19.0", "specs_19_0_base_demo")
                        .unwrap()
                        .port
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(ports[0], ports[1], "one server for both");
    env.rt.stop_all().unwrap();
}

#[test]
fn python_runs_clean_of_the_launchers_python_settings() {
    let env = Env::new(&[], false);
    // A launcher's PYTHONPATH (e.g. /usr/lib/python3/dist-packages) once made pip skip
    // packages it saw there, leaving Odoo without babel when run from the app.
    std::env::set_var("PYTHONPATH", "/leaked/site-packages");
    std::env::set_var("PYTHONHOME", "/leaked/home");
    let db = env.rt.ensure_database("19.0", &[], true);
    let running = db.and_then(|db| env.rt.ensure_running("19.0", &db));
    std::env::remove_var("PYTHONPATH");
    std::env::remove_var("PYTHONHOME");
    running.unwrap();
    env.rt.stop_all().unwrap();
    let log = fs::read_to_string(env.dir.path().join("env.log")).unwrap();
    assert!(!log.contains("/leaked"), "{log}");
    // The user's own site-packages are left out too.
    assert!(log.lines().all(|l| l.ends_with("NOUSERSITE=1")), "{log}");
}

#[test]
fn an_incomplete_python_environment_is_reported_and_not_marked_ready() {
    let env = Env::new(&[], false);
    fs::write(env.dir.path().join("broken-imports"), "").unwrap();
    let err = env.rt.ensure_database("19.0", &[], true).unwrap_err();
    assert!(format!("{err:#}").contains("babel"), "{err:#}");
    assert!(!env
        .rt
        .venv_dir("19.0")
        .join(odoo_runtime::runtime::VENV_READY)
        .exists());
    // Fixed (e.g. network back): the next call rebuilds it.
    fs::remove_file(env.dir.path().join("broken-imports")).unwrap();
    env.rt.ensure_database("19.0", &[], true).unwrap();
    assert_eq!(env.count("python -m venv"), 2);
}

#[test]
fn environments_from_older_app_versions_are_rebuilt_once() {
    let env = Env::new(&[], false);
    let venv = env.rt.venv_dir("19.0");
    fs::create_dir_all(&venv).unwrap();
    fs::write(venv.join(".requirements-installed"), "").unwrap();
    env.rt.ensure_database("19.0", &[], true).unwrap();
    assert_eq!(env.count("python -m venv"), 1, "old marker is not trusted");
    env.rt
        .ensure_database("19.0", &["sale".into()], true)
        .unwrap();
    assert_eq!(env.count("python -m venv"), 1, "the new one is");
}

// --- Databases the business team manages from the app ---

use odoo_runtime::runtime::{session_database, set_session_database, NewDatabase};

fn new_db(label: &str, modules: &[&str]) -> NewDatabase {
    NewDatabase {
        label: label.into(),
        version: "19.0".into(),
        modules: modules.iter().map(|m| m.to_string()).collect(),
        demo_data: true,
        enterprise: false,
    }
}

#[test]
fn a_named_database_is_created_and_listed_with_its_details() {
    let env = Env::new(&[], false);
    let db = env
        .rt
        .create_database(&new_db("Sales demo", &["sale_management"]))
        .unwrap();
    assert!(
        db.name.starts_with("specs_"),
        "only names the app may delete: {}",
        db.name
    );
    assert!(env
        .calls()
        .iter()
        .any(|c| c.contains("-i sale_management") && c.contains(&format!("-d {}", db.name))));

    let list = env.rt.list_databases().unwrap();
    assert_eq!(list.len(), 1);
    let info = &list[0];
    assert_eq!(
        (info.label.as_str(), info.version.as_str()),
        ("Sales demo", "19.0")
    );
    assert_eq!(info.modules, ["sale_management"]);
    assert!(info.demo_data && !info.auto && !info.running);
}

#[test]
fn two_databases_with_the_same_label_get_different_names() {
    let env = Env::new(&[], false);
    let a = env.rt.create_database(&new_db("Test", &[])).unwrap();
    let b = env.rt.create_database(&new_db("Test", &[])).unwrap();
    assert_ne!(a.name, b.name);
}

#[test]
fn databases_made_for_screenshots_are_listed_as_automatic() {
    let env = Env::new(&["specs_19_0_stock_demo"], false);
    fs::write(
        env.dir.path().join("modules-specs_19_0_stock_demo"),
        "stock\n",
    )
    .unwrap();
    env.rt
        .ensure_database("19.0", &["sale".into()], true)
        .unwrap();
    let list = env.rt.list_databases().unwrap();
    let made = list
        .iter()
        .find(|d| d.name == "specs_19_0_sale_demo")
        .unwrap();
    assert!(made.auto);
    assert_eq!(made.modules, ["sale"]);
    // One the catalog never saw (made by an older app version) is still listed,
    // with the apps found in it, looked up only once.
    let older = list
        .iter()
        .find(|d| d.name == "specs_19_0_stock_demo")
        .unwrap();
    assert!(older.auto);
    assert_eq!(older.version, "19.0");
    assert_eq!(older.modules, ["stock"]);
    assert_eq!(older.label, "Auto 19.0: stock");
    let asked = env.count("psql");
    env.rt.list_databases().unwrap();
    assert_eq!(env.count("psql"), asked + 1, "apps are looked up only once");
}

#[test]
fn rename_changes_only_the_label() {
    let env = Env::new(&[], false);
    let db = env.rt.create_database(&new_db("Old", &[])).unwrap();
    env.rt.rename_database(&db.name, "New name").unwrap();
    let list = env.rt.list_databases().unwrap();
    assert_eq!(
        (list[0].name.as_str(), list[0].label.as_str()),
        (db.name.as_str(), "New name")
    );
    assert!(
        env.rt.rename_database(&db.name, "  ").is_err(),
        "a name is needed"
    );
}

#[test]
fn duplicate_copies_the_database_its_files_and_details() {
    let env = Env::new(&[], false);
    let db = env
        .rt
        .create_database(&new_db("Sales demo", &["sale"]))
        .unwrap();
    let files = env
        .rt
        .data_dir()
        .join("filestore")
        .join(&db.name)
        .join("ab");
    fs::create_dir_all(&files).unwrap();
    fs::write(files.join("f"), "x").unwrap();

    let copy = env
        .rt
        .duplicate_database(&db.name, "Sales demo (copy)")
        .unwrap();
    assert_ne!(copy.name, db.name);
    assert!(env.calls().iter().any(|c| c.contains(&format!(
        "CREATE DATABASE \"{}\" TEMPLATE \"{}\"",
        copy.name, db.name
    ))));
    assert_eq!(
        fs::read_to_string(
            env.rt
                .data_dir()
                .join("filestore")
                .join(&copy.name)
                .join("ab/f")
        )
        .unwrap(),
        "x"
    );
    let list = env.rt.list_databases().unwrap();
    let info = list.iter().find(|d| d.name == copy.name).unwrap();
    assert_eq!(
        (info.label.as_str(), info.modules.clone()),
        ("Sales demo (copy)", vec!["sale".to_string()])
    );
}

#[test]
fn reset_rebuilds_a_fresh_database_with_the_same_apps() {
    let env = Env::new(&[], false);
    let db = env
        .rt
        .create_database(&new_db("Sales demo", &["sale"]))
        .unwrap();
    env.rt.reset_database(&db.name).unwrap();
    assert!(env
        .calls()
        .iter()
        .any(|c| c == &format!("dropdb --if-exists {}", db.name)));
    let installs = env
        .calls()
        .iter()
        .filter(|c| c.contains("-i sale") && c.contains(&db.name))
        .count();
    assert_eq!(installs, 2, "installed again after the drop");
    assert_eq!(env.rt.list_databases().unwrap()[0].label, "Sales demo");
}

#[test]
fn delete_forgets_the_database() {
    let env = Env::new(&[], false);
    let db = env.rt.create_database(&new_db("Gone", &[])).unwrap();
    env.rt.delete_database(&db.name).unwrap();
    assert!(env.rt.list_databases().unwrap().is_empty());
}

#[test]
fn only_missing_apps_are_installed_into_a_database() {
    let env = Env::new(&[], false);
    let db = env
        .rt
        .create_database(&new_db("Sales demo", &["sale"]))
        .unwrap();
    env.rt
        .install_missing(&db.name, &["sale".into(), "stock".into()])
        .unwrap();
    let last = env
        .calls()
        .into_iter()
        .rfind(|c| c.starts_with("odoo "))
        .unwrap();
    assert!(
        last.contains("-i stock ") && !last.contains("-i sale"),
        "{last}"
    );
    let before = env.count("odoo ");
    env.rt.install_missing(&db.name, &["stock".into()]).unwrap();
    assert_eq!(env.count("odoo "), before, "nothing to install");
    let info = env.rt.list_databases().unwrap();
    assert_eq!(info[0].modules, ["sale", "stock"]);
}

#[test]
fn a_database_can_be_started_and_stopped_on_its_own() {
    let env = Env::new(&[], false);
    let a = env.rt.create_database(&new_db("A", &[])).unwrap();
    let b = env.rt.create_database(&new_db("B", &[])).unwrap();
    let run_a = env.rt.ensure_running("19.0", &a.name).unwrap();
    let run_b = env.rt.ensure_running("19.0", &b.name).unwrap();
    let running = |name: &str| {
        env.rt
            .list_databases()
            .unwrap()
            .into_iter()
            .find(|d| d.name == name)
            .unwrap()
            .running
    };
    assert!(running(&a.name) && running(&b.name));

    env.rt.stop_database(&a.name).unwrap();
    assert!(!running(&a.name));
    assert!(TcpStream::connect(("127.0.0.1", run_a.port)).is_err());
    assert!(
        TcpStream::connect(("127.0.0.1", run_b.port)).is_ok(),
        "the other keeps running"
    );
}

#[test]
fn a_session_remembers_its_database() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(session_database(dir.path()), None, "Auto until chosen");
    set_session_database(dir.path(), Some("specs_db_sales")).unwrap();
    assert_eq!(
        session_database(dir.path()).as_deref(),
        Some("specs_db_sales")
    );
    set_session_database(dir.path(), None).unwrap();
    assert_eq!(session_database(dir.path()), None);
}

#[test]
fn new_databases_are_numbered_per_version() {
    let env = Env::new(&[], false);
    assert_eq!(env.rt.suggested_label("19.0").unwrap(), "db-19-1");
    let a = env.rt.create_database(&new_db("db-19-1", &[])).unwrap();
    let b = env.rt.create_database(&new_db("My own name", &[])).unwrap();
    assert_eq!(
        (a.name.as_str(), b.name.as_str()),
        ("specs_db_19_1", "specs_db_19_2")
    );
    assert_eq!(env.rt.suggested_label("19.0").unwrap(), "db-19-3");
    // Each version counts on its own.
    assert_eq!(env.rt.suggested_label("18.0").unwrap(), "db-18-1");
    let copy = env.rt.duplicate_database(&a.name, "copy").unwrap();
    assert_eq!(copy.name, "specs_db_19_3");
    // A deleted number is not given again while a higher one exists.
    env.rt.delete_database(&b.name).unwrap();
    assert_eq!(env.rt.suggested_label("19.0").unwrap(), "db-19-4");
}

fn with_enterprise(env: &Env) -> std::path::PathBuf {
    env.rt.ensure_source("19.0").unwrap();
    let enterprise = env
        .rt
        .source_dir("19.0")
        .parent()
        .unwrap()
        .join("enterprise");
    fs::create_dir_all(&enterprise).unwrap();
    enterprise
}

#[test]
fn the_edition_is_chosen_per_database_and_kept_when_it_runs() {
    let env = Env::new(&[], false);
    assert!(!env.rt.enterprise_available("19.0"));
    let mut ee = new_db("EE", &[]);
    ee.enterprise = true;
    let err = env.rt.create_database(&ee).unwrap_err();
    assert!(
        format!("{err:#}").contains("Enterprise is not available for Odoo 19.0"),
        "{err:#}"
    );

    let enterprise = with_enterprise(&env);
    assert!(env.rt.enterprise_available("19.0"));
    let ee = env.rt.create_database(&ee).unwrap();
    let community = env.rt.create_database(&new_db("CE", &[])).unwrap();
    assert!(ee.enterprise && !community.enterprise);
    let path = enterprise.display().to_string();
    let call_for = |db: &str, what: &str| {
        env.calls()
            .into_iter()
            .find(|c| {
                c.starts_with("odoo ") && c.contains(&format!("-d {db} ")) && c.contains(what)
            })
            .unwrap()
    };
    assert!(call_for(&ee.name, "-i").contains(&path));
    assert!(
        !call_for(&community.name, "-i").contains(&path),
        "community even though enterprise is there"
    );

    env.rt.ensure_running("19.0", &community.name).unwrap();
    env.rt.ensure_running("19.0", &ee.name).unwrap();
    assert!(!call_for(&community.name, "--http-port").contains(&path));
    assert!(call_for(&ee.name, "--http-port").contains(&path));
}

// --- Odoo source for the first-run setup ---

use odoo_runtime::runtime::{Edition, NoAccess, SourceState};

#[test]
fn community_and_enterprise_download_with_progress() {
    let env = Env::new(&[], false);
    assert_eq!(
        env.rt.source_state("19.0", Edition::Community),
        SourceState::Missing
    );
    let mut seen = Vec::new();
    env.rt
        .download_source("19.0", Edition::Community, &mut |p| seen.push(p))
        .unwrap();
    assert!(seen.contains(&45) && seen.last() == Some(&100), "{seen:?}");
    assert_eq!(
        env.rt.source_state("19.0", Edition::Community),
        SourceState::Ready { linked: false }
    );
    env.rt
        .download_source("19.0", Edition::Enterprise, &mut |_| {})
        .unwrap();
    assert!(
        env.calls()
            .iter()
            .any(|c| c
                .contains("git@github.com:odoo/enterprise.git +refs/heads/19.0:refs/heads/19.0"))
    );
    assert!(env.rt.enterprise_available("19.0"));
}

#[test]
fn no_access_to_enterprise_is_told_apart_and_leaves_nothing_behind() {
    let env = Env::new(&[], false);
    fs::write(env.dir.path().join("deny"), "").unwrap();
    let err = env
        .rt
        .download_source("19.0", Edition::Enterprise, &mut |_| {})
        .unwrap_err();
    assert!(err.is::<NoAccess>(), "{err:#}");
    assert_eq!(
        env.rt.source_state("19.0", Edition::Enterprise),
        SourceState::Missing
    );
    assert!(!env.rt.enterprise_available("19.0"));
}

#[test]
fn a_folder_the_user_already_has_is_used_after_checking_it() {
    let env = Env::new(&[], false);
    let mine = env.dir.path().join("my-odoo");
    // Not Odoo at all.
    fs::create_dir_all(&mine).unwrap();
    let err = env
        .rt
        .use_folder("19.0", Edition::Community, &mine)
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("not an Odoo source folder"),
        "{err:#}"
    );
    // Odoo, but another version.
    fs::create_dir_all(mine.join("odoo")).unwrap();
    fs::write(mine.join("odoo-bin"), "").unwrap();
    fs::write(
        mine.join("odoo/release.py"),
        "version_info = (18, 0, 0, FINAL, 0)\n",
    )
    .unwrap();
    let err = env
        .rt
        .use_folder("19.0", Edition::Community, &mine)
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("is Odoo 18.0, not 19.0"),
        "{err:#}"
    );
    // The right one.
    fs::write(
        mine.join("odoo/release.py"),
        "version_info = (19, 0, 0, FINAL, 0)\n",
    )
    .unwrap();
    env.rt
        .use_folder("19.0", Edition::Community, &mine)
        .unwrap();
    assert_eq!(
        env.rt.source_state("19.0", Edition::Community),
        SourceState::Ready { linked: true }
    );
    assert_eq!(fs::read_link(env.rt.source_dir("19.0")).unwrap(), mine);

    let ee = env.dir.path().join("my-enterprise");
    fs::create_dir_all(&ee).unwrap();
    assert!(env.rt.use_folder("19.0", Edition::Enterprise, &ee).is_err());
    fs::create_dir_all(ee.join("web_enterprise")).unwrap();
    fs::write(ee.join("web_enterprise/__manifest__.py"), "{}").unwrap();
    env.rt.use_folder("19.0", Edition::Enterprise, &ee).unwrap();
    assert!(env.rt.enterprise_available("19.0"));
}

#[test]
fn reads_saas_versions_from_release_files() {
    use odoo_runtime::runtime::release_version;
    assert_eq!(
        release_version("version_info = (19, 0, 0, FINAL, 0)").as_deref(),
        Some("19.0")
    );
    assert_eq!(
        release_version("version_info = ('saas~18', 2, 0, 'final', 0, '')").as_deref(),
        Some("saas-18.2")
    );
}

// --- A session without a chosen database ---

#[test]
fn a_database_of_the_version_is_reused_with_the_missing_apps_installed() {
    let env = Env::new(&[], false);
    let older = env.rt.create_database(&new_db("Older", &["sale"])).unwrap();
    let recent = env
        .rt
        .create_database(&new_db("Recent", &["sale"]))
        .unwrap();
    env.rt.touch_database(&recent.name).unwrap();
    let made_before = env.count("odoo ");

    let chosen = env
        .rt
        .database_for_version("19.0", &["sale".into(), "stock".into()], true)
        .unwrap();
    assert_eq!(chosen, recent.name, "the most recently used one");
    assert_ne!(chosen, older.name);
    let installs: Vec<_> = env
        .calls()
        .into_iter()
        .filter(|c| c.starts_with("odoo "))
        .skip(made_before)
        .collect();
    assert_eq!(installs.len(), 1, "only the missing app: {installs:?}");
    assert!(installs[0].contains("-i stock "), "{installs:?}");
}

#[test]
fn without_a_database_of_the_version_a_numbered_one_is_made() {
    let env = Env::new(&[], false);
    env.rt
        .create_database(&new_db("Other version", &[]))
        .unwrap(); // 19.0
    let mut v18 = new_db("x", &[]);
    v18.version = "18.0".into();
    let chosen = env
        .rt
        .database_for_version("18.0", &["sale".into()], true)
        .unwrap();
    assert_eq!(chosen, "specs_db_18_1");
    let info = env.rt.database_info(&chosen).unwrap();
    assert_eq!(
        (info.label.as_str(), info.version.as_str()),
        ("db-18-1", "18.0")
    );
    assert_eq!(info.modules, ["sale"]);
    assert!(!info.auto, "a normal database, like one made by hand");
}

#[test]
fn an_old_database_without_demo_data_is_not_picked_when_demo_data_is_wanted() {
    // Made before demo data really loaded on Odoo 19: no suffix, empty.
    let env = Env::new(
        &["specs_19_0_sale_management", "specs_19_0_stock_nodemo"],
        false,
    );
    let list = env.rt.list_databases().unwrap();
    assert!(list.iter().all(|d| !d.demo_data), "{list:?}");
    let chosen = env.rt.database_for_version("19.0", &[], true).unwrap();
    assert_eq!(chosen, "specs_db_19_1", "a new one with demo data");
    // Without demo data, an old one does.
    let plain = env.rt.database_for_version("19.0", &[], false).unwrap();
    assert!(plain.starts_with("specs_19_0_"), "{plain}");
}

// --- Versions: SSH, one repository per edition, a worktree per version ---

#[test]
fn the_app_has_its_own_ssh_key_and_git_uses_only_it() {
    let env = Env::new(&[], false);
    let public = env.rt.ensure_ssh_key().unwrap();
    assert!(public.starts_with("ssh-ed25519 "), "{public}");
    assert_eq!(env.rt.ensure_ssh_key().unwrap(), public, "made once");
    assert_eq!(env.count("ssh-keygen"), 1);
    let key = env.rt.ssh_key_path();
    assert!(
        key.starts_with(env.dir.path()),
        "inside the app's folder, never ~/.ssh: {key:?}"
    );

    env.rt
        .download_source("19.0", Edition::Community, &mut |_| {})
        .unwrap();
    let ssh = fs::read_to_string(env.dir.path().join("ssh.log")).unwrap();
    assert!(ssh.contains(&format!("-i '{}'", key.display())), "{ssh}");
    assert!(
        ssh.contains("IdentitiesOnly=yes") && ssh.contains("BatchMode=yes"),
        "{ssh}"
    );
    assert!(
        ssh.contains("-F /dev/null"),
        "never the person's own SSH config: {ssh}"
    );
}

#[test]
fn versions_share_one_repository_with_a_worktree_each() {
    let env = Env::new(&[], false);
    env.rt
        .download_source("19.0", Edition::Community, &mut |_| {})
        .unwrap();
    env.rt
        .download_source("saas-19.2", Edition::Community, &mut |_| {})
        .unwrap();
    let inits = env
        .calls()
        .iter()
        .filter(|c| c.contains("init --bare"))
        .count();
    assert_eq!(inits, 1, "one repository for every version");
    let worktrees: Vec<_> = env
        .calls()
        .into_iter()
        .filter(|c| c.contains("worktree add"))
        .collect();
    assert_eq!(worktrees.len(), 2);
    assert!(
        worktrees[1].contains(&env.rt.source_dir("saas-19.2").display().to_string()),
        "{worktrees:?}"
    );
    assert!(worktrees[1].contains("--detach"), "{worktrees:?}");
}

#[test]
fn the_versions_on_github_are_listed_newest_first() {
    let env = Env::new(&[], false);
    let versions = env.rt.available_versions().unwrap();
    assert_eq!(
        versions,
        [
            "saas-19.2",
            "saas-19.1",
            "19.0",
            "saas-18.4",
            "18.0",
            "saas-17.4",
            "17.0"
        ],
        "Odoo 17 and newer; no master or development branches"
    );
    assert_eq!(env.rt.latest_saas().unwrap(), "saas-19.2");
    fs::write(env.dir.path().join("deny"), "").unwrap();
    assert!(env.rt.available_versions().is_err());
}

#[test]
fn github_access_is_tested_for_each_edition() {
    let env = Env::new(&[], false);
    let access = env.rt.github_access();
    assert!(access.community && access.enterprise);
    fs::write(env.dir.path().join("deny"), "").unwrap();
    let access = env.rt.github_access();
    assert!(!access.community && !access.enterprise);
}

#[test]
fn a_version_is_updated_after_a_week_but_not_while_it_runs() {
    let env = Env::new(&[], false);
    env.rt
        .download_source("19.0", Edition::Community, &mut |_| {})
        .unwrap();
    assert!(
        !env.rt
            .update_if_stale("19.0", Duration::from_secs(7 * 86400))
            .unwrap(),
        "fresh"
    );
    let before = env.count("git");
    assert!(
        env.rt.update_if_stale("19.0", Duration::ZERO).unwrap(),
        "stale: updated"
    );
    let update: Vec<_> = env.calls().into_iter().skip(before).collect();
    assert!(
        update
            .iter()
            .any(|c| c.contains(" fetch ") && c.contains("19.0")),
        "{update:?}"
    );
    assert!(
        update.iter().any(|c| c.contains("reset --hard")),
        "{update:?}"
    );

    let db = env.rt.create_database(&new_db("Running", &[])).unwrap();
    env.rt.ensure_running("19.0", &db.name).unwrap();
    assert!(
        !env.rt.update_if_stale("19.0", Duration::ZERO).unwrap(),
        "never while it runs"
    );
}

#[test]
fn a_folder_the_user_linked_is_never_updated() {
    let env = Env::new(&[], false);
    let mine = env.dir.path().join("my-odoo");
    fs::create_dir_all(mine.join("odoo")).unwrap();
    fs::write(mine.join("odoo-bin"), "").unwrap();
    fs::write(
        mine.join("odoo/release.py"),
        "version_info = (19, 0, 0, FINAL, 0)\n",
    )
    .unwrap();
    env.rt
        .use_folder("19.0", Edition::Community, &mine)
        .unwrap();
    assert!(!env.rt.update_if_stale("19.0", Duration::ZERO).unwrap());
    assert_eq!(env.count("git"), 0);
}

#[test]
fn removing_a_version_frees_its_files_but_keeps_linked_folders() {
    let env = Env::new(&[], false);
    env.rt
        .download_source("saas-19.2", Edition::Community, &mut |_| {})
        .unwrap();
    fs::create_dir_all(env.rt.venv_dir("saas-19.2")).unwrap();
    let list = env.rt.downloaded_versions();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].version, "saas-19.2");
    env.rt.remove_version("saas-19.2").unwrap();
    assert!(!env.rt.source_dir("saas-19.2").exists());
    assert!(!env.rt.venv_dir("saas-19.2").exists());
    assert!(env.calls().iter().any(|c| c.contains("worktree remove")));

    let mine = env.dir.path().join("my-odoo");
    fs::create_dir_all(mine.join("odoo")).unwrap();
    fs::write(mine.join("odoo-bin"), "").unwrap();
    fs::write(
        mine.join("odoo/release.py"),
        "version_info = (19, 0, 0, FINAL, 0)\n",
    )
    .unwrap();
    env.rt
        .use_folder("19.0", Edition::Community, &mine)
        .unwrap();
    env.rt.remove_version("19.0").unwrap();
    assert!(
        mine.join("odoo-bin").exists(),
        "the user's own folder is untouched"
    );
}

#[test]
fn a_database_says_its_own_version() {
    use odoo_runtime::runtime::version_from_latest;
    assert_eq!(version_from_latest("19.0.1.3").as_deref(), Some("19.0"));
    assert_eq!(
        version_from_latest("saas~19.2.1.3").as_deref(),
        Some("saas-19.2")
    );
    assert_eq!(version_from_latest("nonsense"), None);
}

#[test]
fn the_ai_reads_only_the_sessions_version() {
    let env = Env::new(&[], false);
    env.rt
        .download_source("19.0", Edition::Community, &mut |_| {})
        .unwrap();
    env.rt
        .download_source("saas-19.2", Edition::Community, &mut |_| {})
        .unwrap();
    env.rt
        .download_source("saas-19.2", Edition::Enterprise, &mut |_| {})
        .unwrap();
    let dirs = env.rt.source_dirs_for("saas-19.2");
    assert_eq!(dirs.len(), 2, "{dirs:?}");
    assert!(
        dirs.iter()
            .all(|d| d.to_string_lossy().contains("saas-19.2")),
        "{dirs:?}"
    );
    assert!(env.rt.source_dirs_for("18.0").is_empty(), "not downloaded");
}
