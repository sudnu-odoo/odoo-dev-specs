//! The Odoo source of each version, Community and Enterprise, laid out like odev
//! but light enough for any laptop:
//!
//! ```text
//! sources/repos/odoo, sources/repos/enterprise   one shared repository per edition,
//!                                                holding only the latest commit of
//!                                                each version used (no history)
//! sources/<version>/odoo, .../enterprise         a worktree per version
//! ```
//! Versions share every identical file, so another version costs little download.
//! Everything goes over SSH with the app's own key. Without access the download
//! fails with [`NoAccess`], shown as "not available yet", never as a crash.
//! A folder the person already has (e.g. an odev worktree) can be used instead,
//! and is then never changed by the app.

use std::io::Read;

use anyhow::anyhow;

use super::*;

/// Private repository of Odoo Enterprise.
pub const ENTERPRISE_GIT_URL: &str = "git@github.com:odoo/enterprise.git";

/// Oldest major version the app offers.
const OLDEST_MAJOR: u32 = 17;

/// What a person's GitHub account can read, through the app's key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubAccess {
    pub community: bool,
    pub enterprise: bool,
}

/// Odoo source found on the computer (e.g. an odev worktree), by version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundSource {
    pub version: String,
    pub community: PathBuf,
    pub enterprise: Option<PathBuf>,
}

/// Where people keep Odoo checkouts, relative to their home: each child folder is
/// looked at (odev's worktrees first, so they win over other copies).
const SOURCE_PLACES: [&str; 6] = [
    "odev/worktrees",
    "odoo",
    "src",
    "work",
    "projects",
    "Documents",
];

/// A version on this computer, for the versions list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: String,
    pub community: SourceState,
    pub enterprise: SourceState,
    /// Unix time in milliseconds of the last update (0 when unknown).
    pub updated: u64,
    /// The version's own files: its worktrees and Python environment.
    pub size_bytes: u64,
}

/// A database's version from its `base` module ("19.0.1.3" -> "19.0",
/// "saas~19.2.1.3" -> "saas-19.2"), as odev reads it.
pub fn version_from_latest(latest: &str) -> Option<String> {
    let latest = latest.trim().replace("saas~", "saas-");
    let mut parts = latest.split('.');
    let (major, minor) = (parts.next()?, parts.next()?);
    let numeric = major.trim_start_matches("saas-");
    (!numeric.is_empty()
        && numeric.chars().all(|c| c.is_ascii_digit())
        && !minor.is_empty()
        && minor.chars().all(|c| c.is_ascii_digit()))
    .then(|| format!("{major}.{minor}"))
}

/// Sort key of a branch: "saas-19.2" -> (19, 2), "19.0" -> (19, 0).
fn version_key(branch: &str) -> Option<(u32, u32)> {
    let (major, minor) = branch.trim_start_matches("saas-").split_once('.')?;
    let key = (major.parse().ok()?, minor.parse().ok()?);
    let stable = !branch.starts_with("saas-") && key.1 == 0;
    let saas = branch.starts_with("saas-") && key.1 > 0;
    (stable || saas).then_some(key)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if !meta.is_dir() {
        return meta.len();
    }
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| dir_size(&e.path()))
        .sum()
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// No access to the repository: GitHub refused the key, or the account cannot read it.
fn refused(log: &str) -> bool {
    let lower = log.to_lowercase();
    [
        "permission denied",
        "could not read from remote",
        "repository not found",
        "not found",
        "authentication failed",
    ]
    .iter()
    .any(|m| lower.contains(m))
}

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

    fn repo_dir(&self, edition: Edition) -> PathBuf {
        let name = match edition {
            Edition::Community => "odoo",
            Edition::Enterprise => "enterprise",
        };
        self.root.join("sources").join("repos").join(name)
    }

    fn git_url(&self, edition: Edition) -> &str {
        match edition {
            Edition::Community => &self.odoo_git_url,
            Edition::Enterprise => ENTERPRISE_GIT_URL,
        }
    }

    fn stamp(&self, version: &str, edition: Edition) -> PathBuf {
        self.root
            .join("sources")
            .join(version)
            .join(format!(".updated-{edition:?}").to_lowercase())
    }

    /// Run git with progress on stderr; returns its output, or `NoAccess`.
    fn git_with_progress(
        &self,
        args: &[&str],
        edition: Edition,
        on_progress: &mut dyn FnMut(u8),
    ) -> Result<()> {
        let mut child = self
            .git()
            .args(args)
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
        if child.wait()?.success() {
            return Ok(());
        }
        if refused(&log) {
            return Err(match edition {
                Edition::Enterprise => anyhow::Error::new(NoAccess),
                Edition::Community => anyhow!(
                    "GitHub refused the download: connect your GitHub account in Settings > Odoo versions."
                ),
            });
        }
        bail!("Could not download the Odoo source:\n{}", tail(&log))
    }

    fn git_quiet(&self, args: &[&str]) -> Result<String> {
        let out = self
            .git()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .context("Could not run git. Is it installed?")?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        ensure!(out.status.success(), "git failed:\n{}", tail(&text));
        Ok(text)
    }

    /// Download a version's source, reporting 0..=100 as it goes: the latest commit of
    /// its branch into the edition's shared repository, then a worktree for it.
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
        let repo = self.repo_dir(edition);
        let repo_path = repo.display().to_string();
        if !repo.is_dir() {
            fs::create_dir_all(&repo)?;
            self.git_quiet(&["init", "--bare", "-q", &repo_path])
                .context("Could not prepare the Odoo source folder.")?;
        }
        let refspec = format!("+refs/heads/{version}:refs/heads/{version}");
        self.git_with_progress(
            &[
                "-C",
                &repo_path,
                "fetch",
                "--progress",
                "--depth",
                "1",
                self.git_url(edition),
                &refspec,
            ],
            edition,
            on_progress,
        )?;
        // A worktree left half-made by a stopped download is replaced.
        let target = self.source_path(version, edition);
        if fs::symlink_metadata(&target).is_ok() && !is_link(&target) {
            let _ = fs::remove_dir_all(&target);
        }
        let _ = self.git_quiet(&["-C", &repo_path, "worktree", "prune"]);
        fs::create_dir_all(target.parent().expect("source path has a parent"))?;
        let branch = format!("refs/heads/{version}");
        self.git_quiet(&[
            "-C",
            &repo_path,
            "worktree",
            "add",
            "--detach",
            "--force",
            &target.display().to_string(),
            &branch,
        ])
        .context("Could not set up the Odoo source.")?;
        fs::write(self.stamp(version, edition), now_ms().to_string())?;
        on_progress(100);
        Ok(())
    }

    /// The versions Odoo publishes (17.0 and newer, stable and SaaS), newest first.
    pub fn available_versions(&self) -> Result<Vec<String>> {
        let out = self
            .git_quiet(&["ls-remote", "--heads", self.git_url(Edition::Community)])
            .context("Could not get the list of Odoo versions from GitHub.")?;
        let mut versions: Vec<(String, (u32, u32))> = out
            .lines()
            .filter_map(|l| l.split_whitespace().nth(1)?.strip_prefix("refs/heads/"))
            .filter_map(|b| Some((b.to_string(), version_key(b)?)))
            .filter(|(_, k)| k.0 >= OLDEST_MAJOR)
            .collect();
        versions.sort_by_key(|v| std::cmp::Reverse(v.1));
        versions.dedup();
        Ok(versions.into_iter().map(|(v, _)| v).collect())
    }

    /// The version Odoo Online runs: the newest SaaS release.
    pub fn latest_saas(&self) -> Result<String> {
        self.available_versions()?
            .into_iter()
            .find(|v| v.starts_with("saas-"))
            .context("Odoo publishes no SaaS version right now.")
    }

    /// Can this computer read Odoo's repositories with the app's key?
    pub fn github_access(&self) -> GithubAccess {
        let readable = |edition| {
            self.git_quiet(&["ls-remote", "--heads", self.git_url(edition), "HEAD"])
                .is_ok()
        };
        GithubAccess {
            community: readable(Edition::Community),
            enterprise: readable(Edition::Enterprise),
        }
    }

    /// Whether an Odoo of this version is running right now.
    fn version_running(&self, version: &str) -> bool {
        let catalog = self.load_catalog_versions();
        self.load_state().servers.keys().any(|db| {
            catalog.get(db).cloned().unwrap_or_default() == version
                && self.already_running(db).is_some()
        })
    }

    /// Update the version's source when its last update is older than `max_age`
    /// (Odoo fixes its branches every week). Never while it runs, never a folder
    /// the person linked. Returns whether anything was updated.
    pub fn update_if_stale(&self, version: &str, max_age: Duration) -> Result<bool> {
        validate_version(version)?;
        if self.version_running(version) {
            return Ok(false);
        }
        let mut updated = false;
        for edition in [Edition::Community, Edition::Enterprise] {
            if self.source_state(version, edition) != (SourceState::Ready { linked: false }) {
                continue;
            }
            let last: u64 = fs::read_to_string(self.stamp(version, edition))
                .ok()
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or(0);
            if now_ms().saturating_sub(last) < max_age.as_millis() as u64 {
                continue;
            }
            let path = self.source_path(version, edition).display().to_string();
            self.git_with_progress(
                &[
                    "-C",
                    &path,
                    "fetch",
                    "--depth",
                    "1",
                    self.git_url(edition),
                    version,
                ],
                edition,
                &mut |_| {},
            )?;
            self.git_quiet(&["-C", &path, "reset", "--hard", "-q", "FETCH_HEAD"])?;
            fs::write(self.stamp(version, edition), now_ms().to_string())?;
            updated = true;
        }
        Ok(updated)
    }

    /// The Odoo source in a folder someone picked: an odev-style folder holding `odoo`
    /// (and `enterprise`), or the `odoo` checkout itself (Enterprise next to it).
    pub fn source_at(&self, dir: &Path) -> Option<FoundSource> {
        let (community, enterprise) = if dir.join("odoo/odoo-bin").is_file() {
            (dir.join("odoo"), dir.join("enterprise"))
        } else if dir.join("odoo-bin").is_file() {
            (dir.to_path_buf(), dir.parent()?.join("enterprise"))
        } else {
            return None;
        };
        let version =
            release_version(&fs::read_to_string(community.join("odoo/release.py")).ok()?)?;
        Some(FoundSource {
            version,
            community,
            enterprise: enterprise
                .join("web_enterprise")
                .is_dir()
                .then_some(enterprise),
        })
    }

    /// Odoo source already on the computer, newest version first: folders holding an
    /// `odoo` checkout (with `odoo-bin`) and, when there, an `enterprise` one next to it.
    pub fn find_sources(&self, home: &Path) -> Vec<FoundSource> {
        let mut found: Vec<FoundSource> = Vec::new();
        for place in SOURCE_PLACES {
            let Ok(entries) = fs::read_dir(home.join(place)) else {
                continue;
            };
            let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            dirs.sort();
            for dir in dirs {
                // Only odev-style folders (holding an `odoo` checkout) count here.
                let Some(source) = dir
                    .join("odoo/odoo-bin")
                    .is_file()
                    .then(|| self.source_at(&dir))
                    .flatten()
                else {
                    continue;
                };
                if version_key(&source.version).is_none_or(|k| k.0 < OLDEST_MAJOR)
                    || found.iter().any(|f| f.version == source.version)
                {
                    continue;
                }
                found.push(source);
            }
        }
        found.sort_by_key(|f| std::cmp::Reverse(version_key(&f.version)));
        found
    }

    /// Use the Odoo source found on the computer where it is (nothing downloaded,
    /// the folders never changed); returns the versions newly made available.
    pub fn use_found_sources(&self, home: &Path) -> Result<Vec<String>> {
        let mut used = Vec::new();
        for f in self.find_sources(home) {
            let mut linked = false;
            if self.source_state(&f.version, Edition::Community) == SourceState::Missing {
                self.use_folder(&f.version, Edition::Community, &f.community)?;
                linked = true;
            }
            if let Some(ee) = &f.enterprise {
                if self.source_state(&f.version, Edition::Enterprise) == SourceState::Missing {
                    self.use_folder(&f.version, Edition::Enterprise, ee)?;
                    linked = true;
                }
            }
            if linked {
                used.push(f.version);
            }
        }
        Ok(used)
    }

    /// The versions on this computer.
    pub fn downloaded_versions(&self) -> Vec<VersionInfo> {
        let mut list: Vec<VersionInfo> = fs::read_dir(self.root.join("sources"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let version = e.file_name().to_string_lossy().into_owned();
                validate_version(&version).ok()?;
                let community = self.source_state(&version, Edition::Community);
                let enterprise = self.source_state(&version, Edition::Enterprise);
                (community != SourceState::Missing || enterprise != SourceState::Missing).then(
                    || {
                        let updated = [Edition::Community, Edition::Enterprise]
                            .iter()
                            .filter_map(|ed| fs::read_to_string(self.stamp(&version, *ed)).ok())
                            .filter_map(|t| t.trim().parse().ok())
                            .max()
                            .unwrap_or(0);
                        let size_bytes = dir_size(&self.source_path(&version, Edition::Community))
                            + dir_size(&self.source_path(&version, Edition::Enterprise))
                            + dir_size(&self.venv_dir(&version));
                        VersionInfo {
                            version: version.clone(),
                            community,
                            enterprise,
                            updated,
                            size_bytes,
                        }
                    },
                )
            })
            .collect();
        list.sort_by_key(|v| std::cmp::Reverse(version_key(&v.version)));
        list
    }

    /// Remove a version's files and Python environment (databases stay; they need
    /// the version again to run). A folder the person linked is only unlinked.
    pub fn remove_version(&self, version: &str) -> Result<()> {
        validate_version(version)?;
        ensure!(
            !self.version_running(version),
            "Odoo {version} is running: stop its databases first."
        );
        for edition in [Edition::Community, Edition::Enterprise] {
            let path = self.source_path(version, edition);
            if is_link(&path) {
                fs::remove_file(&path)?;
            } else if path.exists() {
                let repo = self.repo_dir(edition).display().to_string();
                let _ = self.git_quiet(&[
                    "-C",
                    &repo,
                    "worktree",
                    "remove",
                    "--force",
                    &path.display().to_string(),
                ]);
                let _ = fs::remove_dir_all(&path);
            }
            let _ = fs::remove_file(self.stamp(version, edition));
        }
        let _ = fs::remove_dir(self.root.join("sources").join(version));
        let _ = fs::remove_dir_all(self.venv_dir(version));
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
