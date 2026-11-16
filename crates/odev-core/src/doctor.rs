//! First-run environment check: reports whether everything the app needs is in
//! place (odev, PostgreSQL, the Odoo source, Chrome, an AI CLI), with a short
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
    /// Stable identifier, e.g. "odev" or "postgresql".
    pub key: String,
    /// Short human label, e.g. "odev".
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
    /// Path of the `odev` executable, if found.
    pub odev: Option<PathBuf>,
    /// True when PostgreSQL accepts connections (`pg_isready` succeeded).
    pub postgres_ready: bool,
    /// Whether `pg_isready`/`psql` tools exist at all.
    pub postgres_client: bool,
    /// Odoo source directories found via odev.cfg (community first, then enterprise).
    pub odoo_sources: Vec<PathBuf>,
    /// Path of a Chrome/Chromium executable, if found.
    pub chrome: Option<PathBuf>,
    /// Path of the `claude` executable, if found.
    pub claude: Option<PathBuf>,
    /// Path of the `gemini` executable, if found.
    pub gemini: Option<PathBuf>,
}

/// Build the report from a gathered [`Probe`].
pub fn check(probe: &Probe) -> Report {
    Report {
        checks: vec![
            check_odev(probe),
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

fn check_odev(p: &Probe) -> Check {
    match &p.odev {
        Some(path) => ok("odev", "odev", path.display().to_string(), true),
        None => problem(
            "odev",
            "odev",
            Status::Missing,
            "not found".into(),
            "Install odev and make sure it is on your PATH. It creates and runs the local Odoo databases.",
            true,
        ),
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
            "Start the PostgreSQL service (e.g. `sudo service postgresql start`). odev stores the local databases in it.",
            true,
        )
    } else {
        problem(
            "postgresql",
            "PostgreSQL",
            Status::Missing,
            "not installed".into(),
            "Install PostgreSQL. odev stores the local databases in it.",
            true,
        )
    }
}

fn check_source(p: &Probe) -> Check {
    let has = |name: &str| {
        p.odoo_sources
            .iter()
            .any(|s| s.file_name().is_some_and(|n| n == name))
    };
    match (has("odoo"), has("enterprise")) {
        (true, true) => ok("odoo_source", "Odoo source", "community and enterprise".into(), true),
        (true, false) => problem(
            "odoo_source",
            "Odoo source",
            Status::Warning,
            "community only".into(),
            "The enterprise source was not found. Enterprise features cannot be read or screenshotted until it is checked out through odev.",
            true,
        ),
        _ => problem(
            "odoo_source",
            "Odoo source",
            Status::Missing,
            "not found".into(),
            "Check out the Odoo source with odev (set [paths] repositories in odev.cfg). The assistant reads it and runs it for screenshots.",
            true,
        ),
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
    if p.gemini.is_some() {
        present.push("Gemini");
    }
    if present.is_empty() {
        problem(
            "ai",
            "AI assistant",
            Status::Missing,
            "none installed".into(),
            "Install Claude Code (`claude`) or the Gemini CLI (`gemini`) and sign in. The assistant writes the specs.",
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
            odev: Some("/home/u/.local/bin/odev".into()),
            postgres_ready: true,
            postgres_client: true,
            odoo_sources: vec!["/src/odoo".into(), "/src/enterprise".into()],
            chrome: Some("/usr/bin/google-chrome".into()),
            claude: Some("/home/u/.local/bin/claude".into()),
            gemini: None,
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
        for key in ["odev", "postgresql", "odoo_source", "chrome", "ai"] {
            assert_eq!(get(&report, key).status, Status::Ok, "{key}");
        }
        // Every non-ok check offers a hint; ok checks do not need one.
        for c in &report.checks {
            assert_eq!(c.hint.is_some(), c.status != Status::Ok, "{}", c.key);
        }
    }

    #[test]
    fn missing_odev_blocks_and_explains() {
        let report = check(&Probe {
            odev: None,
            ..full()
        });
        let odev = get(&report, "odev");
        assert_eq!(odev.status, Status::Missing);
        assert!(odev.required);
        assert!(odev.hint.as_ref().unwrap().contains("odev"));
        assert!(!report.is_ready());
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
    fn enterprise_source_missing_is_only_a_warning() {
        let report = check(&Probe {
            odoo_sources: vec!["/src/odoo".into()],
            ..full()
        });
        let src = get(&report, "odoo_source");
        assert_eq!(src.status, Status::Warning);
        assert!(src
            .hint
            .as_ref()
            .unwrap()
            .to_lowercase()
            .contains("enterprise"));
        // A warning never blocks first run.
        assert!(report.is_ready());
    }

    #[test]
    fn no_odoo_source_at_all_is_missing() {
        let report = check(&Probe {
            odoo_sources: vec![],
            ..full()
        });
        assert_eq!(get(&report, "odoo_source").status, Status::Missing);
        assert!(!report.is_ready());
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
    fn no_ai_cli_is_missing_but_one_is_enough() {
        let none = check(&Probe {
            claude: None,
            gemini: None,
            ..full()
        });
        assert_eq!(get(&none, "ai").status, Status::Missing);
        assert!(!none.is_ready());

        let only_gemini = check(&Probe {
            claude: None,
            gemini: Some("/g".into()),
            ..full()
        });
        assert_eq!(get(&only_gemini, "ai").status, Status::Ok);
        assert!(only_gemini.is_ready());
    }

    #[test]
    fn the_ai_check_names_which_clis_are_present() {
        let report = check(&Probe {
            claude: Some("/c".into()),
            gemini: Some("/g".into()),
            ..full()
        });
        let ai = get(&report, "ai");
        assert_eq!(ai.status, Status::Ok);
        assert!(
            ai.detail.contains("Claude") && ai.detail.contains("Gemini"),
            "{}",
            ai.detail
        );
    }
}
