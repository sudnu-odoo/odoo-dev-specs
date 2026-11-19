//! The Odoo source of each version, Community and Enterprise, for the first-run
//! setup: downloaded with progress, or a folder the person already has.
//!
//! Enterprise is private: downloading it works once the person's GitHub account has
//! access (signed in with GitHub's `gh`). Without access the download fails with
//! [`NoAccess`], which the setup shows as "not available yet", never as a crash.

use std::io::Read;

use super::*;

/// Private repository of Odoo Enterprise.
pub const ENTERPRISE_GIT_URL: &str = "https://github.com/odoo/enterprise.git";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Edition {
    Community,
    Enterprise,
}

/// Whether a version's source is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum SourceState {
    Missing,
    /// `linked`: a folder the person already had, used where it is.
    Ready {
        linked: bool,
    },
}

/// The GitHub account has no access to the repository (or nobody is signed in).
#[derive(Debug)]
pub struct NoAccess;

impl std::fmt::Display for NoAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Your GitHub account has no access to Odoo Enterprise yet.")
    }
}

impl std::error::Error for NoAccess {}

/// The version a source folder holds, from `odoo/release.py`:
/// `version_info = (19, 0, …)` -> "19.0", `('saas~18', 2, …)` -> "saas-18.2".
pub fn release_version(release_py: &str) -> Option<String> {
    let line = release_py
        .lines()
        .find(|l| l.trim_start().starts_with("version_info"))?;
    let inside = line.split_once('(')?.1;
    let mut parts = inside
        .split(',')
        .map(|p| p.trim().trim_matches(['\'', '"']));
    let (major, minor) = (parts.next()?, parts.next()?);
    let major = major.replace("saas~", "saas-");
    Some(format!("{major}.{minor}"))
}

/// Overall percentage from one line of `git clone --progress`: receiving the
/// objects is most of the work, resolving them the rest.
fn clone_percent(line: &str) -> Option<u8> {
    let percent =
        |rest: &str| -> Option<u32> { rest.trim_start().split('%').next()?.trim().parse().ok() };
    if let Some(rest) = line.split("Receiving objects:").nth(1) {
        return Some((percent(rest)? * 90 / 100) as u8);
    }
    if let Some(rest) = line.split("Resolving deltas:").nth(1) {
        return Some((90 + percent(rest)? / 10) as u8);
    }
    None
}

impl Runtime {
    /// Where a version's edition lives: `sources/<version>/odoo` or `/enterprise`.
    pub fn source_path(&self, version: &str, edition: Edition) -> PathBuf {
        let name = match edition {
            Edition::Community => "odoo",
            Edition::Enterprise => "enterprise",
        };
        self.root.join("sources").join(version).join(name)
    }

    pub fn source_state(&self, version: &str, edition: Edition) -> SourceState {
        let path = self.source_path(version, edition);
        let complete = match edition {
            Edition::Community => path.join("odoo-bin").is_file(),
            Edition::Enterprise => path.join("web_enterprise").is_dir(),
        };
        if !complete {
            return SourceState::Missing;
        }
        SourceState::Ready {
            linked: fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()),
        }
    }

    /// Download a version's source, reporting 0..=100 as it goes. A failed or
    /// stopped download leaves nothing behind.
    pub fn download_source(
        &self,
        version: &str,
        edition: Edition,
        on_progress: &mut dyn FnMut(u8),
    ) -> Result<()> {
        validate_version(version)?;
        if matches!(
            self.source_state(version, edition),
            SourceState::Ready { .. }
        ) {
            on_progress(100);
            return Ok(());
        }
        let target = self.source_path(version, edition);
        let partial = target.with_extension("partial");
        let _ = fs::remove_dir_all(&partial);
        fs::create_dir_all(target.parent().expect("source path has a parent"))?;
        let url = match edition {
            Edition::Community => self.odoo_git_url.as_str(),
            Edition::Enterprise => ENTERPRISE_GIT_URL,
        };
        let mut child = isolated(&self.tools.git)
            .args([
                "clone",
                "--progress",
                "--depth",
                "1",
                "--branch",
                version,
                url,
            ])
            .arg(&partial)
            // Never wait for a password nobody can type: fail, and say why.
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("Could not run git. Is it installed?")?;
        let mut stderr = child.stderr.take().expect("piped");
        let (mut log, mut line, mut buf) = (String::new(), String::new(), [0u8; 4096]);
        let mut last = None;
        loop {
            let n = stderr.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            for c in String::from_utf8_lossy(&buf[..n]).chars() {
                // git redraws its progress with \r.
                if c == '\r' || c == '\n' {
                    if let Some(p) = clone_percent(&line).filter(|p| Some(*p) != last) {
                        on_progress(p);
                        last = Some(p);
                    }
                    log.push_str(&line);
                    log.push('\n');
                    line.clear();
                } else {
                    line.push(c);
                }
            }
        }
        log.push_str(&line);
        let status = child.wait()?;
        if !status.success() {
            let _ = fs::remove_dir_all(&partial);
            let lower = log.to_lowercase();
            if edition == Edition::Enterprise
                && [
                    "not found",
                    "could not read username",
                    "authentication failed",
                    "permission denied",
                ]
                .iter()
                .any(|m| lower.contains(m))
            {
                return Err(anyhow::Error::new(NoAccess));
            }
            bail!(
                "Could not download the Odoo {version} source:\n{}",
                tail(&log)
            );
        }
        let _ = fs::remove_dir_all(&target);
        fs::rename(&partial, &target).context("Could not finish the download.")?;
        on_progress(100);
        Ok(())
    }

    /// Use a folder the person already has, after checking it is the right thing.
    pub fn use_folder(&self, version: &str, edition: Edition, folder: &Path) -> Result<()> {
        validate_version(version)?;
        match edition {
            Edition::Community => {
                let release = fs::read_to_string(folder.join("odoo/release.py")).ok();
                ensure!(
                    folder.join("odoo-bin").is_file() && release.is_some(),
                    "{} is not an Odoo source folder (it has no odoo-bin).",
                    folder.display()
                );
                let found = release.as_deref().and_then(release_version);
                ensure!(
                    found.as_deref() == Some(version),
                    "{} is Odoo {}, not {version}.",
                    folder.display(),
                    found.as_deref().unwrap_or("of an unknown version")
                );
            }
            Edition::Enterprise => ensure!(
                folder.join("web_enterprise").is_dir(),
                "{} is not an Odoo Enterprise folder (it has no web_enterprise).",
                folder.display()
            ),
        }
        let target = self.source_path(version, edition);
        if let Ok(meta) = fs::symlink_metadata(&target) {
            ensure!(
                meta.file_type().is_symlink(),
                "Odoo {version} is already downloaded in the app; it does not need another folder."
            );
            fs::remove_file(&target)?;
        }
        fs::create_dir_all(target.parent().expect("source path has a parent"))?;
        std::os::unix::fs::symlink(folder, &target).context("Could not use that folder.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_covers_receiving_then_resolving() {
        assert_eq!(clone_percent("Receiving objects:  50% (5/10)"), Some(45));
        assert_eq!(
            clone_percent("Receiving objects: 100% (10/10), 4 MiB"),
            Some(90)
        );
        assert_eq!(
            clone_percent("Resolving deltas: 100% (3/3), done."),
            Some(100)
        );
        assert_eq!(clone_percent("Cloning into 'x'..."), None);
    }
}
