//! Where odev keeps the Odoo source code.

use std::path::{Path, PathBuf};

/// Existing Odoo community and enterprise checkouts, read from `[paths] repositories`
/// in odev.cfg (`<repositories>/odoo/odoo` and `<repositories>/odoo/enterprise`).
/// Missing config or folders simply yield fewer entries.
pub fn odoo_source_dirs(config: &Path) -> Vec<PathBuf> {
    let Some(repositories) = std::fs::read_to_string(config)
        .ok()
        .and_then(|text| ini_value(&text, "paths", "repositories"))
    else {
        return vec![];
    };
    let base = PathBuf::from(repositories).join("odoo");
    ["odoo", "enterprise"]
        .iter()
        .map(|name| base.join(name))
        .filter(|p| p.is_dir())
        .collect()
}

/// Value of `key` inside `[section]` of an INI text.
fn ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = name.trim() == section;
        } else if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup(cfg: &str, create: &[&str]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = cfg.replace("{root}", dir.path().to_str().unwrap());
        let path = dir.path().join("odev.cfg");
        fs::write(&path, cfg).unwrap();
        for d in create {
            fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        (dir, path)
    }

    #[test]
    fn both_checkouts_found() {
        let (dir, cfg) = setup(
            "[paths]\ndumps = /x\nrepositories = {root}/repos\n\n[pruning]\ndate = 2031-01-01 00:00:00\n",
            &["repos/odoo/odoo", "repos/odoo/enterprise"],
        );
        let repos = dir.path().join("repos/odoo");
        assert_eq!(
            odoo_source_dirs(&cfg),
            vec![repos.join("odoo"), repos.join("enterprise")]
        );
    }

    #[test]
    fn missing_enterprise_is_skipped() {
        let (dir, cfg) = setup(
            "[paths]\nrepositories = {root}/repos\n",
            &["repos/odoo/odoo"],
        );
        assert_eq!(
            odoo_source_dirs(&cfg),
            vec![dir.path().join("repos/odoo/odoo")]
        );
    }

    #[test]
    fn key_outside_paths_section_is_ignored() {
        let (_d, cfg) = setup(
            "[other]\nrepositories = {root}/repos\n",
            &["repos/odoo/odoo"],
        );
        assert!(odoo_source_dirs(&cfg).is_empty());
    }

    #[test]
    fn missing_config_yields_nothing() {
        assert!(odoo_source_dirs(Path::new("/nonexistent/odev.cfg")).is_empty());
    }
}
