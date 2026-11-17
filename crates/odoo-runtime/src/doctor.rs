//! First-run environment check: reports whether everything the app needs is in
//! place (Python, git, PostgreSQL, the Odoo source, Chrome, an AI CLI), with a short
//! hint for whatever is missing. Pure logic: all system access is injected, so
//! the whole report is testable without touching the real machine.

use std::path::PathBuf;

use serde::Serialize;

/// How a single requirement turned out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Present and usable.
    Ok,
    /// Usable, but something is off (e.g. only one AI CLI, no enterprise source).
    Warning,
    /// Missing; the app cannot do the related work until it is fixed.
    Missing,
}

/// One checked requirement, ready to show in Settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// Stable identifier, e.g. "git" or "postgresql".
    pub key: String,
    /// Short human label, e.g. "PostgreSQL".
    pub label: String,
    pub status: Status,
    /// What was found (a version, a path, "not found"…).
    pub detail: String,
    /// What to do about it, shown only when not `Ok`.
    pub hint: Option<String>,
    /// Whether the app is unusable without this (missing → blocks first run).
    pub required: bool,
}

/// The whole environment report.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    /// True when every required check passed (nothing required is missing).
    pub fn is_ready(&self) -> bool {
        self.checks
            .iter()
            .all(|c| !c.required || c.status != Status::Missing)
    }
}

/// What the machine looks like, gathered by the caller (Tauri side) so this
/// module stays pure. Each field answers one probe.
#[derive(Debug, Clone, Default)]
pub struct Probe {
    /// Path of `python3`, if found (runs Odoo).
    pub python: Option<PathBuf>,
    /// Path of `git`, if found (downloads the Odoo source).
    pub git: Option<PathBuf>,
    /// True when PostgreSQL accepts connections (`pg_isready` succeeded).
    pub postgres_ready: bool,
    /// Whether `pg_isready`/`psql` tools exist at all.
    pub postgres_client: bool,
    /// Odoo source folders already downloaded (`.../sources/<version>/odoo`).
    pub odoo_sources: Vec<PathBuf>,
    /// Path of a Chrome/Chromium executable, if found.
    pub chrome: Option<PathBuf>,
    /// Path of the `claude` executable, if found.
    pub claude: Option<PathBuf>,
}

/// Build the report from a gathered [`Probe`].
pub fn check(probe: &Probe) -> Report {
    Report {
        checks: vec![
            check_program(
                &probe.python,
                "python",
                "Python 3",
                "Install Python 3 (3.10 or newer). It runs the local Odoo.",
            ),
            check_program(
                &probe.git,
                "git",
                "Git",
                "Install git. It downloads the Odoo source the first time a version is used.",
            ),
            check_postgres(probe),
            check_source(probe),
            check_chrome(probe),
            check_ai(probe),
        ],
    }
}

fn ok(key: &str, label: &str, detail: String, required: bool) -> Check {
    Check {
        key: key.into(),
        label: label.into(),
        status: Status::Ok,
        detail,
        hint: None,
        required,
    }
}

fn problem(
    key: &str,
    label: &str,
    status: Status,
    detail: String,
    hint: &str,
    required: bool,
) -> Check {
    Check {
        key: key.into(),
        label: label.into(),
        status,
        detail,
        hint: Some(hint.into()),
        required,
    }
}

fn check_program(path: &Option<PathBuf>, key: &str, label: &str, hint: &str) -> Check {
    match path {
        Some(path) => ok(key, label, path.display().to_string(), true),
        None => problem(key, label, Status::Missing, "not found".into(), hint, true),
    }
}

fn check_postgres(p: &Probe) -> Check {
    if p.postgres_ready {
        ok(
            "postgresql",
            "PostgreSQL",
            "accepting connections".into(),
            true,
        )
    } else if p.postgres_client {
        problem(
            "postgresql",
            "PostgreSQL",
            Status::Missing,
            "not accepting connections".into(),
            "Start the PostgreSQL service (e.g. `sudo service postgresql start`). It stores the local Odoo databases.",
            true,
        )
    } else {
        problem(
            "postgresql",
            "PostgreSQL",
            Status::Missing,
            "not installed".into(),
            "Install PostgreSQL. It stores the local Odoo databases.",
            true,
        )
    }
}

fn check_source(p: &Probe) -> Check {
    // `.../sources/<version>/odoo`: the version is the parent folder's name.
    let versions: Vec<String> = p
        .odoo_sources
        .iter()
        .filter_map(|s| Some(s.parent()?.file_name()?.to_string_lossy().into_owned()))
        .collect();
    if versions.is_empty() {
        problem(
            "odoo_source",
            "Odoo source",
            Status::Warning,
            "not downloaded yet".into(),
            "Downloaded automatically the first time a screenshot of a version is taken (a few hundred MB).",
            false,
        )
    } else {
        ok("odoo_source", "Odoo source", versions.join(", "), false)
    }
}

fn check_chrome(p: &Probe) -> Check {
    match &p.chrome {
        Some(path) => ok("chrome", "Chrome", path.display().to_string(), true),
        None => problem(
            "chrome",
            "Chrome",
            Status::Missing,
            "not found".into(),
            "Install Google Chrome or Chromium. It is used to take the Odoo screenshots.",
            true,
        ),
    }
}

fn check_ai(p: &Probe) -> Check {
    let mut present = Vec::new();
    if p.claude.is_some() {
        present.push("Claude Code");
    }
    if present.is_empty() {
        problem(
            "ai",
            "AI assistant",
            Status::Missing,
            "none installed".into(),
            "Install Claude Code (`claude`) and sign in. The assistant writes the specs.",
            true,
        )
    } else {
        ok("ai", "AI assistant", present.join(", "), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> Probe {
        Probe {
            python: Some("/usr/bin/python3".into()),
            git: Some("/usr/bin/git".into()),
            postgres_ready: true,
            postgres_client: true,
            odoo_sources: vec![
                "/rt/sources/18.0/odoo".into(),
                "/rt/sources/19.0/odoo".into(),
            ],
            chrome: Some("/usr/bin/google-chrome".into()),
            claude: Some("/home/u/.local/bin/claude".into()),
        }
    }

    fn get<'a>(report: &'a Report, key: &str) -> &'a Check {
        report
            .checks
            .iter()
            .find(|c| c.key == key)
            .unwrap_or_else(|| panic!("no check {key}"))
    }

    #[test]
    fn a_full_setup_is_ready() {
        let report = check(&full());
        assert!(report.is_ready());
        let keys: Vec<&str> = report.checks.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            ["python", "git", "postgresql", "odoo_source", "chrome", "ai"]
        );
        assert!(report.checks.iter().all(|c| c.status == Status::Ok));
        // Every non-ok check offers a hint; ok checks do not need one.
        for c in &report.checks {
            assert_eq!(c.hint.is_some(), c.status != Status::Ok, "{}", c.key);
        }
    }

    #[test]
    fn missing_python_or_git_blocks_and_explains() {
        for (probe, key) in [
            (
                Probe {
                    python: None,
                    ..full()
                },
                "python",
            ),
            (
                Probe {
                    git: None,
                    ..full()
                },
                "git",
            ),
        ] {
            let report = check(&probe);
            let c = get(&report, key);
            assert_eq!(c.status, Status::Missing, "{key}");
            assert!(c.required && c.hint.is_some(), "{key}");
            assert!(!report.is_ready(), "{key}");
        }
    }

    #[test]
    fn postgres_not_running_is_missing_even_with_client() {
        let report = check(&Probe {
            postgres_ready: false,
            postgres_client: true,
            ..full()
        });
        let pg = get(&report, "postgresql");
        assert_eq!(pg.status, Status::Missing);
        assert!(pg
            .hint
            .as_ref()
            .unwrap()
            .to_lowercase()
            .contains("postgres"));
        assert!(!report.is_ready());
    }

    #[test]
    fn downloaded_versions_are_listed() {
        assert_eq!(get(&check(&full()), "odoo_source").detail, "18.0, 19.0");
    }

    #[test]
    fn no_source_yet_is_only_a_warning() {
        let report = check(&Probe {
            odoo_sources: vec![],
            ..full()
        });
        let src = get(&report, "odoo_source");
        assert_eq!(src.status, Status::Warning);
        assert!(src.hint.as_ref().unwrap().contains("automatically"));
        // Downloaded on first use: never blocks the first run.
        assert!(report.is_ready());
    }

    #[test]
    fn chrome_missing_blocks_screenshots() {
        let report = check(&Probe {
            chrome: None,
            ..full()
        });
        let chrome = get(&report, "chrome");
        assert_eq!(chrome.status, Status::Missing);
        assert!(chrome.required);
    }

    #[test]
    fn missing_ai_cli_blocks() {
        let none = check(&Probe {
            claude: None,
            ..full()
        });
        assert_eq!(get(&none, "ai").status, Status::Missing);
        assert!(!none.is_ready());
        assert_eq!(get(&check(&full()), "ai").detail, "Claude Code");
    }
}
