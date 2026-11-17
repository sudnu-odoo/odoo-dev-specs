//! Finds AI CLI executables. Apps launched from the desktop menu often lack
//! `~/.local/bin` and nvm folders on PATH, so common install places are searched too.

use std::path::{Path, PathBuf};

/// First executable named `name` in the `PATH`-style list, then in `extra_dirs`.
pub fn find_executable(
    name: &str,
    path_var: Option<&str>,
    extra_dirs: &[PathBuf],
) -> Option<PathBuf> {
    let from_path = path_var.into_iter().flat_map(std::env::split_paths);
    from_path
        .chain(extra_dirs.iter().cloned())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

/// Usual per-user install folders under `home`: `~/.local/bin`, `~/.claude/local`,
/// `~/.npm-global/bin` and every nvm Node version's `bin`, newest version first.
pub fn user_bin_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        home.join(".local/bin"),
        home.join(".claude/local"),
        home.join(".npm-global/bin"),
    ];
    let nvm = home.join(".nvm/versions/node");
    let mut versions: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(&nvm)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().into_owned();
            let version = name
                .trim_start_matches('v')
                .split('.')
                .map(|n| n.parse().ok())
                .collect::<Option<_>>()?;
            Some((version, nvm.join(name).join("bin")))
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    dirs.extend(versions.into_iter().map(|(_, dir)| dir));
    dirs
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn make(path: &Path, executable: bool) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "#!/bin/sh\n").unwrap();
        let mode = if executable { 0o755 } else { 0o644 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn path_wins_over_extra_dirs() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        make(&a.join("claude"), true);
        make(&b.join("claude"), true);
        let path_var = format!("/nonexistent:{}", a.display());
        assert_eq!(
            find_executable("claude", Some(&path_var), &[b]),
            Some(a.join("claude"))
        );
    }

    #[test]
    fn falls_back_to_extra_dirs_and_skips_non_executables() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        make(&a.join("claude"), false);
        make(&b.join("claude"), true);
        let path_var = a.display().to_string();
        assert_eq!(
            find_executable("claude", Some(&path_var), std::slice::from_ref(&b)),
            Some(b.join("claude"))
        );
        assert_eq!(find_executable("no-such-tool", None, &[a, b]), None);
    }

    #[test]
    fn user_bin_dirs_lists_nvm_versions_newest_first() {
        let d = tempfile::tempdir().unwrap();
        for v in ["v18.20.0", "v20.20.2", "v20.9.0"] {
            fs::create_dir_all(d.path().join(".nvm/versions/node").join(v).join("bin")).unwrap();
        }
        let dirs = user_bin_dirs(d.path());
        let nvm = |v: &str| d.path().join(".nvm/versions/node").join(v).join("bin");
        assert_eq!(
            dirs,
            vec![
                d.path().join(".local/bin"),
                d.path().join(".claude/local"),
                d.path().join(".npm-global/bin"),
                nvm("v20.20.2"),
                nvm("v20.9.0"),
                nvm("v18.20.0"),
            ]
        );
    }
}
