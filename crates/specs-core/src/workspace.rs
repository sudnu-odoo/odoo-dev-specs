//! Projects and sessions on disk:
//!
//! ```text
//! <root>/<Project>/<session-id>/
//!     session.json  spec.excalidraw  transcript.jsonl  screenshots/
//! ```
//! Session folders are named by id so renaming a session never moves files.
//!
//! A session's folder never moves either, as the assistants find a conversation
//! again by the folder it ran in. So sessions without a project live in
//! `<root>/.sessions/`, and moving a session to another project only writes the
//! project's name in its `project` file. A deleted project whose folder still
//! holds sessions moved elsewhere keeps the folder, marked `.removed`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::fsutil::write_atomic;
use crate::transcript::TranscriptEntry;

const META: &str = "session.json";
const SCENE: &str = "spec.excalidraw";
const TRANSCRIPT: &str = "transcript.jsonl";
const SCREENSHOTS: &str = "screenshots";
/// Where sessions without a project are created.
const LOOSE: &str = ".sessions";
/// In a session folder: the project it belongs to ("" = none), when it was moved.
const MEMBER: &str = "project";
/// In a project folder: the project was deleted (its folder only holds others' sessions).
const REMOVED: &str = ".removed";
/// In a project folder: the project is pinned.
const PINNED: &str = ".pinned";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub provider: String,
    /// Unix time in milliseconds.
    pub created_at: u64,
    pub updated_at: u64,
    /// True once the AI CLI has a conversation for this session (later turns resume it).
    pub started: bool,
    /// The provider's own conversation id, for CLIs that pick it themselves (Antigravity).
    #[serde(default)]
    pub conversation_id: Option<String>,
    /// The local Odoo database the session's screenshots use (`None` = Auto, not
    /// chosen yet). Kept in its own file, which the screenshot tool also writes.
    #[serde(default, skip_deserializing)]
    pub database: Option<String>,
    /// Where the client's Odoo runs: "online" (Odoo Online) or "sh" (Odoo.sh);
    /// `None` until chosen. The AI is told it with every message.
    #[serde(default)]
    #[ts(type = "\"online\" | \"sh\" | null")]
    pub hosting: Option<String>,
    /// The client's Odoo version: an Odoo branch, e.g. "19.0" or "saas-19.2".
    #[serde(default)]
    pub version: Option<String>,
    /// Pinned at the top of the sidebar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pinned: Option<bool>,
}

/// The hostings a session can be for.
pub const HOSTINGS: [&str; 2] = ["online", "sh"];

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ProjectInfo {
    pub name: String,
    /// Pinned at the top of the sidebar.
    #[ts(optional)]
    pub pinned: Option<bool>,
    /// Most recently updated first.
    pub sessions: Vec<SessionMeta>,
}

pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `~/OdooSpecs`
    pub fn default_root() -> PathBuf {
        dirs::home_dir().unwrap_or_default().join("OdooSpecs")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// All projects sorted by name, after the sessions without a project (named "",
    /// only when there are some); creates the root folder on first use.
    pub fn list(&self) -> Result<Vec<ProjectInfo>> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("creating {}", self.root.display()))?;
        let mut projects: Vec<ProjectInfo> = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_dir()
                && validate_name(&name).is_ok()
                && !entry.path().join(REMOVED).exists()
            {
                projects.push(ProjectInfo {
                    pinned: entry.path().join(PINNED).exists().then_some(true),
                    name,
                    sessions: vec![],
                });
            }
        }
        projects.sort_by_key(|p| p.name.to_lowercase());
        let mut loose = Vec::new();
        for (dir, project) in self.session_folders()? {
            let Ok(meta) = read_meta(&dir) else { continue };
            match projects.iter_mut().find(|p| p.name == project) {
                Some(p) => p.sessions.push(meta),
                None => loose.push(meta),
            }
        }
        for p in &mut projects {
            p.sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
        }
        if !loose.is_empty() {
            loose.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
            projects.insert(
                0,
                ProjectInfo {
                    pinned: None,
                    name: String::new(),
                    sessions: loose,
                },
            );
        }
        Ok(projects)
    }

    /// Every session folder, with the project it belongs to ("" = none).
    fn session_folders(&self) -> Result<Vec<(PathBuf, String)>> {
        let mut found = Vec::new();
        for folder in fs::read_dir(&self.root)? {
            let folder = folder?;
            if !folder.file_type()?.is_dir() {
                continue;
            }
            let folder_name = folder.file_name().to_string_lossy().into_owned();
            for entry in fs::read_dir(folder.path())? {
                let dir = entry?.path();
                if dir.join(META).is_file() {
                    let project = member_of(&dir, &folder_name);
                    found.push((dir, project));
                }
            }
        }
        Ok(found)
    }

    /// Pin a session at the top of the sidebar, or unpin it. Not a change to the
    /// session: its place among the recent ones stays.
    pub fn pin_session(&self, id: &str, pinned: bool) -> Result<()> {
        let dir = self.session_dir("", id)?;
        let mut meta = read_meta(&dir)?;
        meta.pinned = pinned.then_some(true);
        write_meta(&dir, &meta)
    }

    /// Pin a project at the top of the sidebar, or unpin it.
    pub fn pin_project(&self, project: &str, pinned: bool) -> Result<()> {
        let mark = self.project_dir(project)?.join(PINNED);
        if pinned {
            write_atomic(&mark, b"")
        } else if mark.exists() {
            fs::remove_file(&mark).context("unpinning the project")
        } else {
            Ok(())
        }
    }

    /// Put a session in another project ("" = none). Its folder stays where it is.
    ///
    /// # Errors
    /// Unknown session or project.
    pub fn move_session(&self, id: &str, project: &str) -> Result<()> {
        let dir = self.session_dir("", id)?;
        let project = project.trim();
        if !project.is_empty() {
            self.project_dir(project)?;
        }
        write_atomic(&dir.join(MEMBER), project.as_bytes())
    }

    /// Delete a project with its sessions. Sessions moved out of it are kept.
    ///
    /// # Errors
    /// Unknown project, or files that cannot be deleted.
    pub fn delete_project(&self, project: &str) -> Result<()> {
        let dir = self.project_dir(project)?;
        for (session, member) in self.session_folders()? {
            if member == project.trim() {
                fs::remove_dir_all(&session)
                    .with_context(|| format!("deleting {}", session.display()))?;
            }
        }
        let others = fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .any(|e| e.path().join(META).is_file());
        if others {
            let _ = fs::remove_file(dir.join(PINNED));
            write_atomic(&dir.join(REMOVED), b"")
        } else {
            fs::remove_dir_all(&dir).with_context(|| format!("deleting {}", dir.display()))
        }
    }

    pub fn create_project(&self, name: &str) -> Result<()> {
        let name = validate_name(name)?;
        let dir = self.root.join(name);
        if dir.join(REMOVED).exists() {
            return fs::remove_file(dir.join(REMOVED)).context("restoring the project");
        }
        ensure!(!dir.exists(), "A project named \"{name}\" already exists.");
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))
    }

    pub fn create_session(
        &self,
        project: &str,
        title: &str,
        provider: &str,
    ) -> Result<SessionMeta> {
        let project_dir = if project.trim().is_empty() {
            self.root.join(LOOSE)
        } else {
            self.project_dir(project)?
        };
        let now = now_ms();
        let latest = self.latest_target(project);
        let meta = SessionMeta {
            id: Uuid::new_v4().to_string(),
            title: validate_title(title)?,
            provider: provider.to_string(),
            created_at: now,
            updated_at: now,
            started: false,
            conversation_id: None,
            database: None,
            // Like the client's latest session: a client that moved hosting moves
            // every new session with it, while older specs keep theirs.
            hosting: latest.as_ref().and_then(|m| m.hosting.clone()),
            version: latest.and_then(|m| m.version),
            pinned: None,
        };
        let dir = project_dir.join(&meta.id);
        fs::create_dir_all(dir.join(SCREENSHOTS))?;
        write_atomic(&dir.join(SCENE), empty_scene().as_bytes())?;
        write_meta(&dir, &meta)?;
        Ok(meta)
    }

    pub fn session(&self, project: &str, id: &str) -> Result<SessionMeta> {
        read_meta(&self.session_dir(project, id)?)
    }

    /// Folder of an existing session, found by its id (`_project` is not needed: a
    /// session moved to another project keeps its folder). Rejects ids that could
    /// escape the root.
    pub fn session_dir(&self, _project: &str, id: &str) -> Result<PathBuf> {
        let id = Uuid::parse_str(id)
            .context("Invalid session id.")?
            .to_string();
        for folder in fs::read_dir(&self.root).context("This session no longer exists.")? {
            let dir = folder?.path().join(&id);
            if dir.join(META).is_file() {
                return Ok(dir);
            }
        }
        bail!("This session no longer exists.")
    }

    pub fn rename_session(&self, project: &str, id: &str, title: &str) -> Result<SessionMeta> {
        let title = validate_title(title)?;
        self.update_meta(project, id, |m| m.title = title)
    }

    pub fn delete_session(&self, project: &str, id: &str) -> Result<()> {
        let dir = self.session_dir(project, id)?;
        fs::remove_dir_all(&dir).with_context(|| format!("deleting {}", dir.display()))
    }

    /// Record that the AI conversation exists, and bump `updated_at`.
    pub fn mark_started(&self, project: &str, id: &str) -> Result<SessionMeta> {
        self.update_meta(project, id, |m| m.started = true)
    }

    /// Tie the session to the provider answering its first message.
    pub fn set_provider(&self, project: &str, id: &str, provider: &str) -> Result<SessionMeta> {
        self.update_meta(project, id, |m| m.provider = provider.to_string())
    }

    /// Remember the provider's conversation id, so later turns can resume it.
    pub fn set_conversation_id(
        &self,
        project: &str,
        id: &str,
        conversation_id: &str,
    ) -> Result<SessionMeta> {
        self.update_meta(project, id, |m| {
            m.conversation_id = Some(conversation_id.to_string());
        })
    }

    /// Set where the client's Odoo runs for this session.
    pub fn set_hosting(
        &self,
        project: &str,
        id: &str,
        hosting: Option<&str>,
    ) -> Result<SessionMeta> {
        if let Some(h) = hosting {
            ensure!(HOSTINGS.contains(&h), "Unknown hosting: {h}");
        }
        self.update_meta(project, id, |m| m.hosting = hosting.map(String::from))
    }

    /// Set the client's Odoo version for this session (an Odoo branch name).
    pub fn set_version(
        &self,
        project: &str,
        id: &str,
        version: Option<&str>,
    ) -> Result<SessionMeta> {
        if let Some(v) = version {
            ensure!(
                valid_version(v),
                "\"{v}\" is not an Odoo version (like 19.0 or saas-19.2)."
            );
        }
        self.update_meta(project, id, |m| m.version = version.map(String::from))
    }

    /// The project's most recently changed session that has a hosting: new sessions
    /// start with its hosting and version.
    fn latest_target(&self, project: &str) -> Option<SessionMeta> {
        if project.trim().is_empty() {
            return None;
        }
        self.list()
            .ok()?
            .into_iter()
            .find(|p| p.name == project.trim())?
            .sessions
            .into_iter()
            .filter(|m| m.hosting.is_some())
            .max_by_key(|m| (m.updated_at, m.created_at))
    }

    /// Choose the database for the session's screenshots (`None` = Auto).
    pub fn set_database(&self, project: &str, id: &str, database: Option<&str>) -> Result<()> {
        odoo_runtime::runtime::set_session_database(&self.session_dir(project, id)?, database)
    }

    pub fn read_scene(&self, project: &str, id: &str) -> Result<String> {
        let path = self.session_dir(project, id)?.join(SCENE);
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    }

    /// Replace the diagram. Must be a valid `.excalidraw` document.
    pub fn write_scene(&self, project: &str, id: &str, json: &str) -> Result<()> {
        let v: Value = serde_json::from_str(json).context("The diagram is not valid JSON.")?;
        ensure!(
            v["type"] == "excalidraw",
            "The diagram is not an Excalidraw document."
        );
        write_atomic(&self.session_dir(project, id)?.join(SCENE), json.as_bytes())
    }

    pub fn append_transcript(
        &self,
        project: &str,
        id: &str,
        entries: &[TranscriptEntry],
    ) -> Result<()> {
        let mut buf = String::new();
        for e in entries {
            buf.push_str(&serde_json::to_string(e)?);
            buf.push('\n');
        }
        let path = self.session_dir(project, id)?.join(TRANSCRIPT);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(buf.as_bytes())
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Unreadable lines are skipped so one bad line never hides a whole chat.
    pub fn read_transcript(&self, project: &str, id: &str) -> Result<Vec<TranscriptEntry>> {
        let path = self.session_dir(project, id)?.join(TRANSCRIPT);
        if !path.exists() {
            return Ok(vec![]);
        }
        Ok(fs::read_to_string(&path)?
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }

    fn project_dir(&self, project: &str) -> Result<PathBuf> {
        let dir = self.root.join(validate_name(project)?);
        if !dir.is_dir() || dir.join(REMOVED).exists() {
            bail!("The project \"{project}\" does not exist.");
        }
        Ok(dir)
    }

    fn update_meta(
        &self,
        project: &str,
        id: &str,
        change: impl FnOnce(&mut SessionMeta),
    ) -> Result<SessionMeta> {
        let dir = self.session_dir(project, id)?;
        let mut meta = read_meta(&dir)?;
        change(&mut meta);
        meta.updated_at = now_ms().max(meta.updated_at + 1);
        write_meta(&dir, &meta)?;
        Ok(meta)
    }
}

/// The project of the session in `dir`, inside the folder named `folder`.
fn member_of(dir: &Path, folder: &str) -> String {
    match fs::read_to_string(dir.join(MEMBER)) {
        Ok(project) => project.trim().to_string(),
        Err(_) if folder == LOOSE => String::new(),
        Err(_) => folder.to_string(),
    }
}

/// Project names become folder names: keep them to one safe path component.
fn validate_name(name: &str) -> Result<&str> {
    let name = name.trim();
    ensure!(!name.is_empty(), "The name cannot be empty.");
    ensure!(!name.starts_with('.'), "The name cannot start with a dot.");
    ensure!(
        !name.contains(['/', '\\', '\0']),
        "The name cannot contain / or \\."
    );
    Ok(name)
}

fn validate_title(title: &str) -> Result<String> {
    let title = title.trim();
    ensure!(!title.is_empty(), "The title cannot be empty.");
    Ok(title.to_string())
}

/// "19.0" or "saas-19.2".
fn valid_version(v: &str) -> bool {
    let number = v.strip_prefix("saas-").unwrap_or(v);
    number.split_once('.').is_some_and(|(major, minor)| {
        !major.is_empty()
            && !minor.is_empty()
            && major.chars().all(|c| c.is_ascii_digit())
            && minor.chars().all(|c| c.is_ascii_digit())
    })
}

fn read_meta(dir: &Path) -> Result<SessionMeta> {
    let path = dir.join(META);
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut meta: SessionMeta =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    meta.database = odoo_runtime::runtime::session_database(dir);
    Ok(meta)
}

fn write_meta(dir: &Path, meta: &SessionMeta) -> Result<()> {
    write_atomic(
        &dir.join(META),
        serde_json::to_string_pretty(meta)?.as_bytes(),
    )
}

fn empty_scene() -> String {
    json!({
        "type": "excalidraw",
        "version": 2,
        "source": "odoo-dev-specs",
        "elements": [],
        "appState": { "viewBackgroundColor": "#ffffff" },
        "files": {}
    })
    .to_string()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
