//! Server management against a fake `odev` (no PostgreSQL or Odoo needed):
//! `run` starts a tiny Python web server on the requested port.

use std::fs;
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use odev_core::server::{database_name, Odev};
use tempfile::TempDir;

/// Written once, before any process starts (avoids "Text file busy").
/// It logs each call to `$ODEV_LOG`; `list -1` prints `$ODEV_DATABASES`;
/// `create` fails when `$ODEV_FAIL_CREATE` is set.
fn fake_odev() -> PathBuf {
    static FAKE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    FAKE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odev");
        let script = r#"#!/bin/sh
echo "$*" >> "$ODEV_LOG"
case "$1" in
  list) printf '%s\n' $ODEV_DATABASES ;;
  create) if [ -n "$ODEV_FAIL_CREATE" ]; then echo "Module sale_typo not found" >&2; exit 1; fi ;;
  run) port=""; prev=""
       for a in "$@"; do [ "$prev" = "--http-port" ] && port="$a"; prev="$a"; done
       exec python3 -m http.server "$port" --bind 127.0.0.1 ;;
esac
"#;
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        (dir, path)
    })
    .1
    .clone()
}

struct Env {
    odev: Odev,
    dir: TempDir,
    _serial: MutexGuard<'static, ()>,
}

/// Tests here write a script and then run it. Running them one at a time keeps
/// another test's process start from holding the fresh script open ("Text file busy").
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Each test gets its own log and state via a wrapper that sets the env vars.
fn env(databases: &str, fail_create: bool) -> Env {
    let serial = serial();
    let fake = fake_odev();
    let dir = tempfile::tempdir().unwrap();
    let wrapper = dir.path().join("odev-wrapper");
    let fail = if fail_create {
        "export ODEV_FAIL_CREATE=1\n"
    } else {
        ""
    };
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexport ODEV_LOG='{}'\nexport ODEV_DATABASES='{databases}'\n{fail}exec '{}' \"$@\"\n",
            dir.path().join("calls.log").display(),
            fake.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let odev = Odev::new(&wrapper, dir.path().join("state"));
    Env {
        odev,
        dir,
        _serial: serial,
    }
}

impl Env {
    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join("calls.log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.odev.stop_all();
    }
}

#[test]
fn database_names_are_stable_and_readable() {
    let _serial = serial();
    let a = database_name("19.0", &["stock".into(), "sale_management".into()], true);
    let b = database_name(
        "19.0",
        &["sale_management".into(), "stock".into(), "stock".into()],
        true,
    );
    assert_eq!(a, b, "order and duplicates do not matter");
    assert_eq!(a, "specs_19_0_sale_management_stock");
    assert_eq!(database_name("18.0", &[], true), "specs_18_0_base");
    // Demo and no-demo are different databases, so the two never clash.
    assert_eq!(database_name("18.0", &[], false), "specs_18_0_base_nodemo");
    assert_ne!(
        database_name("19.0", &["sale".into()], true),
        database_name("19.0", &["sale".into()], false)
    );
    let many: Vec<String> = (0..20).map(|i| format!("module_number_{i}")).collect();
    let long = database_name("19.0", &many, true);
    assert!(long.len() <= 63, "PostgreSQL limit: {long}");
    assert_ne!(long, database_name("19.0", &many[..19], true));
}

#[test]
fn creates_a_missing_database_with_its_modules() {
    let env = env("other_db", false);
    let name = env
        .odev
        .ensure_database("19.0", &["sale_management".into()], true)
        .unwrap();
    assert_eq!(name, "specs_19_0_sale_management");
    assert_eq!(
        env.calls(),
        // Pulling first records the weekly update, so create never stops to ask about it.
        [
            "list -1",
            "pull -f -V 19.0",
            "create -f -V 19.0 specs_19_0_sale_management -i sale_management --stop-after-init"
        ]
    );
}

#[test]
fn without_demo_data_uses_a_separate_database() {
    let env = env("", false);
    let name = env
        .odev
        .ensure_database("19.0", &["sale_management".into()], false)
        .unwrap();
    assert_eq!(name, "specs_19_0_sale_management_nodemo");
    assert_eq!(
        env.calls().last().unwrap(),
        "create -f -V 19.0 specs_19_0_sale_management_nodemo -i sale_management --without-demo=all --stop-after-init"
    );
}

#[test]
fn reuses_an_existing_database() {
    let env = env("specs_19_0_sale_management", false);
    env.odev
        .ensure_database("19.0", &["sale_management".into()], true)
        .unwrap();
    assert_eq!(env.calls(), ["list -1"]);
}

#[test]
fn creation_failures_explain_why() {
    let env = env("", true);
    let err = env
        .odev
        .ensure_database("19.0", &["sale_typo".into()], true)
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("Module sale_typo not found"),
        "{err:#}"
    );
    // The half-created database is removed, so it is never mistaken for a ready one.
    assert_eq!(
        env.calls().last().unwrap(),
        "delete -f specs_19_0_sale_typo"
    );
}

#[test]
fn starts_once_reuses_and_stops() {
    let env = env("", false);
    let first = env.odev.ensure_running("19.0", "specs_19_0_base").unwrap();
    assert!(
        TcpStream::connect(("127.0.0.1", first.port)).is_ok(),
        "server answers"
    );
    assert_eq!(first.url(), format!("http://127.0.0.1:{}", first.port));

    // A later process (new Odev on the same state) finds the running server.
    let again = Odev::new(fake_odev(), env.dir.path().join("state"))
        .ensure_running("19.0", "specs_19_0_base")
        .unwrap();
    assert_eq!(again, first);
    let runs = env.calls().iter().filter(|c| c.starts_with("run ")).count();
    assert_eq!(runs, 1, "{:?}", env.calls());
    assert_eq!(env.calls()[0], "pull -f -V 19.0");
    assert!(env.calls()[1].starts_with("run -f specs_19_0_base --http-port "));

    env.odev.stop_all().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(
        TcpStream::connect(("127.0.0.1", first.port)).is_err(),
        "server stopped"
    );
}

#[test]
fn a_dead_server_is_restarted() {
    let env = env("", false);
    let first = env.odev.ensure_running("19.0", "specs_19_0_base").unwrap();
    env.odev.stop_all().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let second = env.odev.ensure_running("19.0", "specs_19_0_base").unwrap();
    assert!(TcpStream::connect(("127.0.0.1", second.port)).is_ok());
    let _ = first;
}
