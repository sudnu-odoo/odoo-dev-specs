//! Keeps odev from asking what to do with old databases.
//!
//! odev prompts interactively when databases are older than `[pruning] date`
//! in `~/.config/odev/odev.cfg`. Pushing that date far into the future silences
//! the prompt. The file is patched line by line so every other key, comment and
//! the original formatting stay exactly as they were.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{Local, Months, NaiveDateTime};

const DATE_FMT: &str = "%Y-%m-%d %H:%M:%S";
/// How far ahead the pruning date is pushed.
pub const YEARS_AHEAD: u32 = 5;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The existing date was already far enough ahead.
    AlreadyOk(String),
    /// The date was changed (old value, if any → new value).
    Updated { old: Option<String>, new: String },
    /// No odev.cfg on this machine (odev not installed / never run).
    NoConfig,
}

pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("odev").join("odev.cfg"))
}

/// Ensure the pruning date in `path` is at least `now + YEARS_AHEAD`.
/// A `.bak` copy of the original is written before any change.
pub fn ensure_pruning_date(path: &Path, now: NaiveDateTime) -> Result<Outcome> {
    if !path.exists() {
        return Ok(Outcome::NoConfig);
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let threshold = now
        .checked_add_months(Months::new(12 * YEARS_AHEAD))
        .context("date overflow")?;
    let new_value = threshold.format(DATE_FMT).to_string();

    let (patched, old) = patch(&original, threshold, &new_value);
    let Some(patched) = patched else {
        return Ok(Outcome::AlreadyOk(old.unwrap_or_default()));
    };

    fs::copy(path, path.with_extension("cfg.bak")).context("writing backup")?;
    // Write atomically so odev never sees a half-written file.
    let tmp = path.with_extension("cfg.tmp");
    fs::write(&tmp, patched).context("writing temp config")?;
    fs::rename(&tmp, path).context("replacing config")?;
    Ok(Outcome::Updated {
        old,
        new: new_value,
    })
}

/// Convenience wrapper used at app start-up.
pub fn ensure_default() -> Result<Outcome> {
    match default_config_path() {
        Some(p) => ensure_pruning_date(&p, Local::now().naive_local()),
        None => Ok(Outcome::NoConfig),
    }
}

/// Returns `(Some(new_text), old_value)` when a change is needed,
/// `(None, old_value)` when the current date is already far enough ahead.
fn patch(
    text: &str,
    threshold: NaiveDateTime,
    new_value: &str,
) -> (Option<String>, Option<String>) {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut section_start = None; // index of the `[pruning]` header line
    let mut date_line = None;

    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            if section_start.is_some() {
                break; // left the [pruning] section
            }
            if line[1..line.len() - 1].trim() == "pruning" {
                section_start = Some(i);
            }
            continue;
        }
        if section_start.is_some() {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "date" {
                    date_line = Some((i, v.trim().to_string()));
                }
            }
        }
    }

    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let new_line = format!("date = {new_value}{eol}");
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();

    match (section_start, date_line) {
        (Some(_), Some((idx, old))) => {
            let far_enough = NaiveDateTime::parse_from_str(&old, DATE_FMT)
                .map(|d| d >= threshold)
                .unwrap_or(false);
            if far_enough {
                return (None, Some(old));
            }
            out[idx] = new_line;
            (Some(out.concat()), Some(old))
        }
        (Some(start), None) => {
            // Section exists but has no date key: add it right after the header.
            out.insert(start + 1, new_line);
            (Some(out.concat()), None)
        }
        (None, _) => {
            let mut s = text.to_string();
            if !s.is_empty() && !s.ends_with('\n') {
                s.push_str(eol);
            }
            if !s.is_empty() {
                s.push_str(eol);
            }
            s.push_str(&format!("[pruning]{eol}{new_line}"));
            (Some(s), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-11-16 12:00:00", DATE_FMT).unwrap()
    }

    const SAMPLE: &str = "[paths]\ndumps = /x\n\n[pruning]\ndate = 2025-01-01 00:00:00\n\n[update]\ndate = 2026-11-16 09:21:58\nmode = never\n";

    fn run(content: &str) -> (Outcome, String, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("odev.cfg");
        fs::write(&p, content).unwrap();
        let o = ensure_pruning_date(&p, now()).unwrap();
        let s = fs::read_to_string(&p).unwrap();
        (o, s, dir)
    }

    #[test]
    fn updates_old_date_and_keeps_everything_else() {
        let (o, s, dir) = run(SAMPLE);
        assert_eq!(
            o,
            Outcome::Updated {
                old: Some("2025-01-01 00:00:00".into()),
                new: "2031-11-16 12:00:00".into()
            }
        );
        assert_eq!(
            s,
            SAMPLE.replace("date = 2025-01-01 00:00:00", "date = 2031-11-16 12:00:00")
        );
        // The [update] date must not be touched.
        assert!(s.contains("[update]\ndate = 2026-11-16 09:21:58\n"));
        assert_eq!(
            fs::read_to_string(dir.path().join("odev.cfg.bak")).unwrap(),
            SAMPLE
        );
    }

    #[test]
    fn leaves_far_future_date_alone() {
        let content = SAMPLE.replace("2025-01-01 00:00:00", "2040-01-01 00:00:00");
        let (o, s, dir) = run(&content);
        assert_eq!(o, Outcome::AlreadyOk("2040-01-01 00:00:00".into()));
        assert_eq!(s, content);
        assert!(!dir.path().join("odev.cfg.bak").exists());
    }

    #[test]
    fn adds_missing_section() {
        let (o, s, _d) = run("[paths]\ndumps = /x\n");
        assert!(matches!(o, Outcome::Updated { old: None, .. }));
        assert_eq!(
            s,
            "[paths]\ndumps = /x\n\n[pruning]\ndate = 2031-11-16 12:00:00\n"
        );
    }

    #[test]
    fn adds_missing_key_in_existing_section() {
        let (_, s, _d) = run("[pruning]\n[user]\nx = 1\n");
        assert_eq!(s, "[pruning]\ndate = 2031-11-16 12:00:00\n[user]\nx = 1\n");
    }

    #[test]
    fn garbage_date_is_replaced() {
        let (o, _, _d) = run("[pruning]\ndate = soon\n");
        assert!(matches!(o, Outcome::Updated { .. }));
    }

    #[test]
    fn missing_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let o = ensure_pruning_date(&dir.path().join("nope.cfg"), now()).unwrap();
        assert_eq!(o, Outcome::NoConfig);
    }
}
