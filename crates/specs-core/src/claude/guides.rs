//! The consultant guides: short Markdown files the assistant reads only when a
//! question needs them (impact of a change, what a hosting allows, an upgrade).
//! They ship inside the app and are written into each session's `guides/` folder,
//! so every assistant can read them and an app update refreshes them.

use std::path::Path;

use anyhow::Result;

use crate::fsutil::write_atomic;

/// The folder, inside a session, that holds the guides.
pub const DIR: &str = "guides";

/// Every guide: its file name and its text.
pub const GUIDES: &[(&str, &str)] = &[
    ("analysis.md", include_str!("guides/analysis.md")),
    ("impact-check.md", include_str!("guides/impact-check.md")),
    ("odoo-online.md", include_str!("guides/odoo-online.md")),
    ("odoo-sh.md", include_str!("guides/odoo-sh.md")),
    ("moving-to-sh.md", include_str!("guides/moving-to-sh.md")),
    (
        "upgrade-impact.md",
        include_str!("guides/upgrade-impact.md"),
    ),
];

/// Write the guides into `session_dir/guides`, only those missing or changed.
///
/// # Errors
/// The folder or a file cannot be written.
pub fn install(session_dir: &Path) -> Result<()> {
    let dir = session_dir.join(DIR);
    std::fs::create_dir_all(&dir)?;
    for (name, text) in GUIDES {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*text) {
            write_atomic(&path, text.as_bytes())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_every_guide_and_repairs_an_edited_one() {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path()).unwrap();
        for (name, text) in GUIDES {
            assert_eq!(
                &std::fs::read_to_string(dir.path().join(DIR).join(name)).unwrap(),
                text
            );
        }
        let edited = dir.path().join(DIR).join("impact-check.md");
        std::fs::write(&edited, "changed").unwrap();
        install(dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(edited).unwrap(), GUIDES[1].1);
    }

    #[test]
    fn the_prompt_lists_every_guide() {
        for (name, _) in GUIDES {
            assert!(
                super::super::SYSTEM_PROMPT.contains(name),
                "not in the index: {name}"
            );
        }
    }

    #[test]
    fn guides_never_give_estimates_or_name_internal_tools() {
        for (name, text) in GUIDES {
            let lower = text.to_lowercase();
            for word in ["odev", "ps-tools", "ps_tools", "presales", "worktree"] {
                assert!(!lower.contains(word), "{name} mentions {word}");
            }
        }
    }
}
