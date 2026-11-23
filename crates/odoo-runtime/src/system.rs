//! What Ubuntu must provide for the app (PostgreSQL with a database user, Python
//! that can make virtualenvs, Git, Chrome), whether it really works,
//! and the one script that fixes whatever is missing, run once as root.
//!
//! The checks look at what works, not only at what is installed: PostgreSQL can be
//! installed but stopped, or running without a database user for this person.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

/// Oldest Python the supported Odoo versions run on.
pub const MIN_PYTHON: (u32, u32) = (3, 10);

/// Google's package for Chrome (no Ubuntu package ships it).
const CHROME_DEB: &str =
    "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";

/// What was found on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemProbe {
    /// The PostgreSQL client and server are installed.
    pub postgres_installed: bool,
    /// The server accepts connections.
    pub postgres_running: bool,
    /// This person can connect (their database user exists).
    pub postgres_user: bool,
    /// `python3 --version`, when Python is there.
    pub python: Option<(u32, u32)>,
    /// Python can make virtualenvs with pip (Ubuntu's `python3-venv`).
    pub python_venv: bool,
    pub git: bool,
    pub chrome: bool,
}

/// One thing the app needs, as the setup screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct Requirement {
    pub key: &'static str,
    pub label: &'static str,
    pub ok: bool,
    /// What is wrong, in plain words (empty when ok).
    pub problem: String,
}

/// Everything the app needs from the system, with what is missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct SystemReport {
    pub requirements: Vec<Requirement>,
    /// Nothing to install or start.
    pub ready: bool,
}

/// One step of the fix script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// `apt-get install` these Ubuntu packages.
    Packages(Vec<&'static str>),
    /// Download and install Google Chrome.
    Chrome,
    /// Start PostgreSQL now and at every boot.
    StartPostgres,
    /// Create this person's database user (allowed to create databases).
    CreateUser,
}

fn requirement(key: &'static str, label: &'static str, problem: Option<String>) -> Requirement {
    Requirement {
        key,
        label,
        ok: problem.is_none(),
        problem: problem.unwrap_or_default(),
    }
}

/// What works and what does not, in the setup screen's words.
pub fn report(p: &SystemProbe) -> SystemReport {
    let python_problem = match p.python {
        None => Some("Python 3 is not installed.".to_string()),
        Some(v) if v < MIN_PYTHON => Some(format!(
            "Python {}.{} is too old (needs {}.{} or newer).",
            v.0, v.1, MIN_PYTHON.0, MIN_PYTHON.1
        )),
        Some(_) if !p.python_venv => Some("Python cannot make environments yet.".to_string()),
        Some(_) => None,
    };
    let postgres_problem = if !p.postgres_installed {
        Some("PostgreSQL is not installed.".to_string())
    } else if !p.postgres_running {
        Some("PostgreSQL is not running.".to_string())
    } else if !p.postgres_user {
        Some("The app cannot log in to PostgreSQL yet (your database user or its access is missing).".to_string())
    } else {
        None
    };
    let requirements = vec![
        requirement("postgresql", "PostgreSQL", postgres_problem),
        requirement("python", "Python", python_problem),
        requirement(
            "git",
            "Git",
            (!p.git).then(|| "Git is not installed.".into()),
        ),
        requirement(
            "chrome",
            "Chrome",
            (!p.chrome).then(|| "Chrome is not installed.".into()),
        ),
    ];
    let ready = requirements.iter().all(|r| r.ok);
    SystemReport {
        requirements,
        ready,
    }
}

/// The steps that fix what is missing, in order; empty when nothing is.
pub fn plan(p: &SystemProbe) -> Vec<Fix> {
    let mut packages = Vec::new();
    if !p.postgres_installed {
        packages.push("postgresql");
    }
    if p.python.is_none_or(|v| v < MIN_PYTHON) {
        packages.push("python3");
    }
    if !p.python_venv || p.python.is_none() {
        packages.push("python3-venv");
    }
    if !p.git {
        packages.push("git");
    }
    let mut fixes = Vec::new();
    if !packages.is_empty() {
        fixes.push(Fix::Packages(packages));
    }
    if !p.chrome {
        fixes.push(Fix::Chrome);
    }
    if !p.postgres_installed || !p.postgres_running {
        fixes.push(Fix::StartPostgres);
    }
    if !p.postgres_user {
        fixes.push(Fix::CreateUser);
    }
    fixes
}

/// Where PostgreSQL's access file is: the server says, else its running process
/// (its config file or data folder), else any cluster's folder. Empty if none.
const FIND_HBA: &str = r#"find_hba() {
  f=$(runuser -u postgres -- psql -tAc 'SHOW hba_file' 2>/dev/null || true)
  if [ -n "$f" ] && [ -f "$f" ]; then echo "$f"; return; fi
  args=$(ps -o args= -C postgres 2>/dev/null | grep -- ' -D ' | head -n 1 || true)
  conf=$(printf '%s' "$args" | sed -n 's/.*config_file=\([^ ]*\).*/\1/p')
  if [ -n "$conf" ] && [ -f "$(dirname "$conf")/pg_hba.conf" ]; then echo "$(dirname "$conf")/pg_hba.conf"; return; fi
  data=$(printf '%s' "$args" | sed -n 's/.* -D \([^ ]*\).*/\1/p')
  if [ -n "$data" ] && [ -f "$data/pg_hba.conf" ]; then echo "$data/pg_hba.conf"; return; fi
  ls /etc/postgresql/*/*/pg_hba.conf 2>/dev/null | sort -V | tail -n 1 || true
}
"#;

/// A user name safe to put in the script (Linux user names are like this).
pub fn valid_user(user: &str) -> bool {
    !user.is_empty()
        && user.len() <= 64
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !user.starts_with('-')
}

/// The script that applies `fixes`, to run as root. Safe to run again: every step
/// skips what is already done. `user` is the person using the app.
pub fn script(fixes: &[Fix], user: &str) -> String {
    assert!(valid_user(user), "not a user name: {user}");
    let mut s = String::from(
        // A language the machine has not installed (e.g. LANG=id_ID.UTF-8 without its
        // pack) makes PostgreSQL's install skip creating its database folder: use the
        // neutral one every Ubuntu has.
        "set -eu\nexport DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8 LC_ALL=C.UTF-8\nunset LANGUAGE\n\
         say() { echo \"== $*\"; }\n",
    );
    let mut packages: Vec<&str> = fixes
        .iter()
        .flat_map(|f| match f {
            Fix::Packages(p) => p.clone(),
            _ => vec![],
        })
        .collect();
    let chrome = fixes.contains(&Fix::Chrome);
    if chrome {
        // To fetch Chrome's package.
        packages.extend(["wget", "ca-certificates"]);
    }
    if !packages.is_empty() {
        s.push_str("say Installing programs\napt-get update\n");
        s.push_str(&format!("apt-get install -y {}\n", packages.join(" ")));
    }
    if chrome {
        s.push_str(&format!(
            "say Installing Chrome\ntmp=$(mktemp -d)\n\
             wget -q -O \"$tmp/chrome.deb\" {CHROME_DEB}\n\
             apt-get install -y \"$tmp/chrome.deb\"\nrm -rf \"$tmp\"\n"
        ));
    }
    if fixes.contains(&Fix::StartPostgres) {
        // systemd on a laptop; plain `service` where there is none (containers).
        // Installed but without a database folder (a cluster), nothing can start:
        // make the usual one first.
        s.push_str(
            "if command -v pg_lsclusters >/dev/null && [ -z \"$(pg_lsclusters -h 2>/dev/null)\" ]; then\n\
             \x20 say Creating the PostgreSQL database folder\n\
             \x20 pg_createcluster \"$(ls /usr/lib/postgresql | sort -V | tail -n 1)\" main\n\
             fi\n\
             say Starting PostgreSQL\n\
             if [ -d /run/systemd/system ]; then systemctl enable --now postgresql; \
             else service postgresql start; fi\n",
        );
    }
    if fixes.contains(&Fix::CreateUser) {
        s.push_str(FIND_HBA);
        // Some machines' pg_hba.conf (changed by another tool) has no rule for local
        // connections ("no pg_hba.conf entry for host [local]"): let this user, and
        // PostgreSQL's own admin, in from this computer as Ubuntu does by default, at
        // the top so they win. Each rule is added once; the old file is backed up.
        s.push_str(&format!(
            "if ! runuser -u '{user}' -- psql -d postgres -tAc 'SELECT 1' >/dev/null 2>&1; then\n\
             \x20 hba=$(find_hba)\n\
             \x20 if [ -z \"$hba\" ]; then\n\
             \x20   echo \"Could not find PostgreSQL's access file (pg_hba.conf). Is PostgreSQL running?\" >&2; exit 1\n\
             \x20 fi\n\
             \x20 for who in postgres '{user}'; do\n\
             \x20   if ! grep -Eq \"^local[[:space:]]+all[[:space:]]+(all|$who)[[:space:]]+peer\" \"$hba\"; then\n\
             \x20     say Letting $who into PostgreSQL\n\
             \x20     [ -f \"$hba.before-odoo-dev-specs\" ] || cp -p \"$hba\" \"$hba.before-odoo-dev-specs\"\n\
             \x20     {{ echo \"local   all   $who   peer   # added by Odoo Dev Specs\"; cat \"$hba\"; }} > \"$hba.new\"\n\
             \x20     cat \"$hba.new\" > \"$hba\"; rm -f \"$hba.new\"\n\
             \x20   fi\n\
             \x20 done\n\
             \x20 if [ -d /run/systemd/system ]; then systemctl reload postgresql; else service postgresql reload; fi\n\
             fi\n"
        ));
        s.push_str(&format!(
            "say Creating your database user\n\
             if ! runuser -u postgres -- psql -tAc \"SELECT 1 FROM pg_roles WHERE rolname='{user}'\" | grep -q 1; then\n\
             \x20 runuser -u postgres -- createuser --superuser '{user}'\nfi\n"
        ));
    }
    s.push_str("say Done\n");
    s
}

// --- Probing the real machine ---------------------------------------------------

fn run(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    // A stuck program (e.g. a server that never answers) must not hang the check.
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(30))
            }
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    out.status.success().then(|| {
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

/// "Python 3.12.3" -> (3, 12).
pub fn parse_python_version(text: &str) -> Option<(u32, u32)> {
    let version = text.split_whitespace().nth(1)?;
    let mut parts = version.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// Look at the real machine.
pub fn probe() -> SystemProbe {
    let python = run("python3", &["--version"]).and_then(|v| parse_python_version(v.trim()));
    // Ubuntu's python3 without python3-venv has no ensurepip: venvs come without pip.
    let python_venv =
        python.is_some() && run("python3", &["-c", "import ensurepip, venv"]).is_some();
    let postgres_installed = on_path("psql") || Path::new("/usr/lib/postgresql").is_dir();
    let postgres_running = run("pg_isready", &["-q"]).is_some();
    let postgres_user =
        postgres_running && run("psql", &["-d", "postgres", "-Atc", "SELECT 1"]).is_some();
    SystemProbe {
        postgres_installed,
        postgres_running,
        postgres_user,
        python,
        python_venv,
        git: on_path("git"),
        chrome: [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ]
        .iter()
        .any(|c| on_path(c)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_good() -> SystemProbe {
        SystemProbe {
            postgres_installed: true,
            postgres_running: true,
            postgres_user: true,
            python: Some((3, 12)),
            python_venv: true,
            git: true,
            chrome: true,
        }
    }

    #[test]
    fn a_ready_machine_needs_nothing_and_asks_for_no_password() {
        assert!(report(&all_good()).ready);
        assert!(plan(&all_good()).is_empty());
    }

    #[test]
    fn a_fresh_ubuntu_gets_everything_in_one_go() {
        let fresh = SystemProbe {
            python: Some((3, 12)),
            ..SystemProbe::default()
        };
        let fixes = plan(&fresh);
        assert_eq!(
            fixes,
            [
                Fix::Packages(vec!["postgresql", "python3-venv", "git"]),
                Fix::Chrome,
                Fix::StartPostgres,
                Fix::CreateUser
            ]
        );
        let s = script(&fixes, "ana");
        assert!(
            s.contains("apt-get install -y postgresql python3-venv git wget ca-certificates"),
            "{s}"
        );
        assert!(s.contains("google-chrome-stable_current_amd64.deb"));
        assert!(s.contains("createuser --superuser 'ana'"));
        let report = report(&fresh);
        assert!(!report.ready);
        let problems: Vec<_> = report
            .requirements
            .iter()
            .filter(|r| !r.ok)
            .map(|r| r.key)
            .collect();
        assert_eq!(problems, ["postgresql", "python", "git", "chrome"]);
    }

    #[test]
    fn the_classic_stuck_point_only_needs_the_database_user() {
        let p = SystemProbe {
            postgres_user: false,
            ..all_good()
        };
        assert_eq!(plan(&p), [Fix::CreateUser]);
        let s = script(&plan(&p), "ana");
        assert!(!s.contains("apt-get"), "nothing to install: {s}");
        assert_eq!(
            report(&p).requirements[0].problem,
            "The app cannot log in to PostgreSQL yet (your database user or its access is missing)."
        );
    }

    #[test]
    fn the_user_is_let_in_even_when_postgresql_has_no_local_rule_for_them() {
        // A pg_hba.conf changed by another tool: "no pg_hba.conf entry for host [local]".
        let s = script(&[Fix::CreateUser], "ana");
        let rule = s.find("for who in postgres 'ana'").expect(&s);
        let user = s.find("createuser --superuser 'ana'").unwrap();
        assert!(
            rule < user,
            "access first, or even creating the user fails: {s}"
        );
        // Only when they cannot connect, each rule once, with a backup, then reloaded.
        assert!(s.contains("runuser -u 'ana' -- psql -d postgres"), "{s}");
        assert!(s.contains("/etc/postgresql/*/*/pg_hba.conf"), "{s}");
        assert!(s.contains("Could not find PostgreSQL's access file"), "{s}");
        assert!(s.contains(".before-odoo-dev-specs"), "{s}");
        assert!(s.contains("systemctl reload postgresql"), "{s}");
    }

    #[test]
    fn a_stopped_postgresql_is_started_and_kept_starting() {
        let p = SystemProbe {
            postgres_running: false,
            postgres_user: false,
            ..all_good()
        };
        assert_eq!(plan(&p), [Fix::StartPostgres, Fix::CreateUser]);
        assert!(script(&plan(&p), "ana").contains("systemctl enable --now postgresql"));
    }

    #[test]
    fn python_without_venv_or_too_old_is_fixed() {
        let no_venv = SystemProbe {
            python_venv: false,
            ..all_good()
        };
        assert_eq!(plan(&no_venv), [Fix::Packages(vec!["python3-venv"])]);
        let old = SystemProbe {
            python: Some((3, 8)),
            ..all_good()
        };
        assert!(report(&old).requirements[1].problem.contains("too old"));
    }

    #[test]
    fn every_script_is_valid_bash() {
        // Each fix on its own, all of them, and none: bash must accept the script.
        let all = [
            Fix::Packages(vec!["postgresql", "git"]),
            Fix::Chrome,
            Fix::StartPostgres,
            Fix::CreateUser,
        ];
        let mut cases: Vec<Vec<Fix>> = all.iter().map(|f| vec![f.clone()]).collect();
        cases.push(all.to_vec());
        cases.push(vec![]);
        for fixes in cases {
            let script = script(&fixes, "ana.maria-2");
            let out = std::process::Command::new("bash")
                .args(["-n", "-c", &script])
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{fixes:?}: {}\n{script}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }

    #[test]
    fn the_script_never_takes_an_unsafe_user_name() {
        assert!(valid_user("sudnu"));
        assert!(valid_user("ana.maria-2"));
        for bad in ["", "a b", "x'; rm -rf /", "-rf", "a$b"] {
            assert!(!valid_user(bad), "{bad}");
        }
    }

    #[test]
    fn reads_python_versions() {
        assert_eq!(parse_python_version("Python 3.12.3"), Some((3, 12)));
        assert_eq!(parse_python_version("nonsense"), None);
    }
}
