//! Gathers what the machine looks like into a [`Probe`], so `odoo_runtime::doctor`
//! can turn it into the environment report shown on first run and in Settings.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use odoo_runtime::doctor::Probe;
use odoo_runtime::runtime::{Runtime, Tools};
use specs_core::locate::{find_executable, user_bin_dirs};
use specs_core::providers::{self, ProviderStatus};
use specs_core::settings::Provider;

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

/// Is each AI provider installed and logged in? Asks the CLIs themselves.
pub fn provider_status() -> Vec<ProviderStatus> {
    let home = dirs::home_dir().unwrap_or_default();
    let path = std::env::var("PATH").ok();
    let bins = user_bin_dirs(&home);
    let find = |name: &str| find_executable(name, path.as_deref(), &bins);

    let claude = find("claude");
    let claude_in = claude.as_ref().is_some_and(|p| {
        output_within(p, &["auth", "status", "--json"], LOGIN_CHECK)
            .is_some_and(|out| providers::claude_logged_in(&out))
    });
    let agy = find("agy");
    // agy has no status command; listing models needs a login and fails without one.
    let agy_in = agy
        .as_ref()
        .is_some_and(|p| output_within(p, &["models"], LOGIN_CHECK).is_some());

    vec![
        status(Provider::Claude, claude, claude_in),
        status(Provider::Antigravity, agy, agy_in),
    ]
}

/// How long a CLI may take to say whether it is logged in.
const LOGIN_CHECK: Duration = Duration::from_secs(10);

fn status(provider: Provider, program: Option<PathBuf>, logged_in: bool) -> ProviderStatus {
    ProviderStatus {
        provider,
        name: provider.display_name().into(),
        installed: program.is_some(),
        logged_in,
        program,
    }
}

/// Stdout of a successful run that finished in time; `None` otherwise.
fn output_within(program: &Path, args: &[&str], limit: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(status) if status.success() => break,
            Some(_) => return None,
            None if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Run `command` in a new terminal window, so the user can follow a login.
pub fn open_in_terminal(command: &[String]) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_default();
    let path = std::env::var("PATH").ok();
    let bins = user_bin_dirs(&home);
    let terminal = providers::TERMINALS
        .iter()
        .find_map(|t| find_executable(t, path.as_deref(), &bins))
        .ok_or(
            "No terminal app found. Run the login command yourself: ".to_string()
                + &command.join(" "),
        )?;
    Command::new(&terminal)
        .args(providers::terminal_args(&terminal, command))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open {}: {e}", terminal.display()))
}
