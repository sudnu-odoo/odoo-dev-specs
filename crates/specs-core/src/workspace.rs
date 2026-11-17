//! Projects and sessions on disk:
//!
//! ```text
//! <root>/<Project>/<session-id>/
//!     session.json  spec.excalidraw  transcript.jsonl  screenshots/
//! ```
//! Session folders are named by id so renaming a session never moves files.

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectInfo {
    pub name: String,
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

    /// All projects sorted by name; creates the root folder on first use.
    pub fn list(&self) -> Result<Vec<ProjectInfo>> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("creating {}", self.root.display()))?;
        let mut projects = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type()?.is_dir() || validate_name(&name).is_err() {
                continue;
            }
            let mut sessions: Vec<SessionMeta> = fs::read_dir(entry.path())?
                .filter_map(|e| read_meta(&e.ok()?.path()).ok())
                .collect();
            sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
            projects.push(ProjectInfo { name, sessions });
        }
        projects.sort_by_key(|p| p.name.to_lowercase());
        Ok(projects)
    }

    pub fn create_project(&self, name: &str) -> Result<()> {
        let name = validate_name(name)?;
        let dir = self.root.join(name);
        ensure!(!dir.exists(), "A project named \"{name}\" already exists.");
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))
    }

    pub fn create_session(
        &self,
        project: &str,
        title: &str,
        provider: &str,
    ) -> Result<SessionMeta> {
        let project_dir = self.project_dir(project)?;
        let now = now_ms();
        let meta = SessionMeta {
            id: Uuid::new_v4().to_string(),
            title: validate_title(title)?,
            provider: provider.to_string(),
            created_at: now,
            updated_at: now,
            started: false,
            conversation_id: None,
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

    /// Folder of an existing session. Rejects ids and names that could escape the root.
    pub fn session_dir(&self, project: &str, id: &str) -> Result<PathBuf> {
        let id = Uuid::parse_str(id)
            .context("Invalid session id.")?
            .to_string();
        let dir = self.project_dir(project)?.join(id);
        ensure!(dir.join(META).is_file(), "This session no longer exists.");
        Ok(dir)
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
        if !dir.is_dir() {
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

fn read_meta(dir: &Path) -> Result<SessionMeta> {
    let path = dir.join(META);
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
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
