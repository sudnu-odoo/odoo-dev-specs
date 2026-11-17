//! The Odoo runtime against fake `git`, `python`, `psql` and `dropdb` programs:
//! no network, PostgreSQL or real Odoo needed. The fake Odoo "server" is Python's
//! static web server on the requested port.

use std::fs;
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

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

        // git clone --depth 1 --branch V URL DEST: a tiny "Odoo" checkout.
        script(
            &bin.join("git"),
            &format!(
                r#"echo "git $*" >> '{log}'
for last; do :; done
mkdir -p "$last/addons" "$last/odoo/addons"
echo '#!/bin/sh' > "$last/odoo-bin"
printf 'psycopg2==2.9.9\npython-ldap==3.4.4\nlxml==5.2.1\n' > "$last/requirements.txt""#
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
  echo "$db" >> '{dbs}'
  sleep 0.3
  {fail}
  touch '{d}/ready-'"$db"
  exit 0
fi
exec /usr/bin/python3 -m http.server "$port" --bind 127.0.0.1"#,
                d = d.display()
            ),
        );
        script(
            &bin.join("psql"),
            &format!(r#"echo "psql" >> '{log}'; cat '{dbs}'"#),
        );
        script(
            &bin.join("dropdb"),
            &format!(
                r#"echo "dropdb $*" >> '{log}'
for last; do :; done
grep -vx "$last" '{dbs}' > '{dbs}.tmp'; mv '{dbs}.tmp' '{dbs}'"#
            ),
        );

        let tools = Tools {
            git: bin.join("git"),
            python: bin.join("python3"),
            psql: bin.join("psql"),
            dropdb: bin.join("dropdb"),
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

    assert_eq!(
        env.calls()
            .into_iter()
            .find(|c| c.starts_with("git"))
            .unwrap(),
        "git clone --depth 1 --branch 19.0 https://example.invalid/odoo.git ".to_string()
            + &env.rt.source_dir("19.0").display().to_string()
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
    assert_eq!(env.count("git"), 1);
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
