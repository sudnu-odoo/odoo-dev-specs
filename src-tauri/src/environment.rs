//! Gathers what the machine looks like into a [`Probe`], so `odoo_runtime::doctor`
//! can turn it into the environment report shown on first run and in Settings.

use std::process::Command;

use odoo_runtime::doctor::Probe;
use odoo_runtime::runtime::{Runtime, Tools};
use specs_core::locate::{find_executable, user_bin_dirs};

/// Chrome/Chromium executable names, in the order we prefer them.
const CHROME_NAMES: [&str; 4] = [
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
];

/// Inspect the real machine.
pub fn probe() -> Probe {
    let home = dirs::home_dir().unwrap_or_default();
    let path = std::env::var("PATH").ok();
    let bins = user_bin_dirs(&home);
    let find = |name: &str| find_executable(name, path.as_deref(), &bins);

    Probe {
        python: find("python3"),
        git: find("git"),
        postgres_ready: postgres_ready(),
        postgres_client: find("pg_isready").is_some() || find("psql").is_some(),
        odoo_sources: Runtime::new(Runtime::default_root(), Tools::default()).source_dirs(),
        chrome: CHROME_NAMES.iter().find_map(|name| find(name)),
        claude: find("claude"),
    }
}

/// `pg_isready` exits 0 when PostgreSQL is accepting connections.
fn postgres_ready() -> bool {
    run_ok("pg_isready", &["-q"])
}

fn run_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// `agy`, when it is installed and has been run/logged in at least once (its state
/// folder exists). Checked on every call so logging in takes effect immediately.
pub fn find_antigravity() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let path = std::env::var("PATH").ok();
    let program = find_executable("agy", path.as_deref(), &user_bin_dirs(&home))?;
    home.join(".gemini/antigravity-cli")
        .is_dir()
        .then_some(program)
}
