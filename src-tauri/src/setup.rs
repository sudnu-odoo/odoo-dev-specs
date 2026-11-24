//! The first-run setup: what is missing on this computer, installing it with one
//! password prompt, downloading the Odoo source in the background (progress sent as
//! events), signing in to GitHub for Enterprise, and using folders people already have.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Odoo fixes its branches every week: a version is brought up to date after that.
const WEEK: Duration = Duration::from_secs(7 * 24 * 3600);

use odoo_runtime::runtime::{
    Edition, FoundSource, NoAccess, Runtime as OdooRuntime, SourceState, VersionInfo,
};
use odoo_runtime::system::{self, SystemProbe, SystemReport};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};

use crate::commands::AppState;

/// Name of the event carrying a [`TaskUpdate`] to the UI.
pub const SETUP_EVENT: &str = "setup-progress";

/// Runs the fix script as root; returns its output, or a sentence for the user.
pub type Elevate = dyn Fn(&str) -> Result<String, String> + Send + Sync;

/// Is GitHub's tool there, and can the app's key read Odoo's repositories?
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubStatus {
    pub installed: bool,
    /// The app's key reads Odoo Community (any GitHub account can).
    pub connected: bool,
    /// It also reads Odoo Enterprise (the account has access).
    pub enterprise: bool,
}

/// How the setup reaches the machine; swappable in tests.
pub struct SetupHooks {
    pub probe: Arc<dyn Fn() -> SystemProbe + Send + Sync>,
    pub elevate: Arc<Elevate>,
    pub github: Arc<dyn Fn(&OdooRuntime) -> GithubStatus + Send + Sync>,
    /// The person's Linux user name, for their database user.
    pub user: String,
    /// Their home folder, where Odoo checkouts are looked for (e.g. odev's worktrees).
    pub home: PathBuf,
}

impl Default for SetupHooks {
    fn default() -> Self {
        Self {
            probe: Arc::new(system::probe),
            elevate: Arc::new(run_as_root),
            github: Arc::new(github_status),
            user: std::env::var("USER").unwrap_or_default(),
            home: dirs::home_dir().unwrap_or_default(),
        }
    }
}

/// Where a long setup step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum TaskState {
    Running,
    Done,
    Failed,
    /// GitHub refused: the person's account cannot read that repository.
    NoAccess,
}

/// A long setup step, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct Task {
    pub state: TaskState,
    /// 0..=100 while downloading; none while waiting on something without progress.
    pub progress: Option<u8>,
    /// What is happening, or what went wrong, in plain words.
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdate {
    pub key: String,
    pub task: Task,
}

#[derive(Default)]
pub struct SetupState {
    pub hooks: SetupHooks,
    tasks: Mutex<HashMap<String, Task>>,
}

impl SetupState {
    pub fn new(hooks: SetupHooks) -> Self {
        Self {
            hooks,
            tasks: Mutex::default(),
        }
    }

    fn running(&self, key: &str) -> bool {
        self.tasks
            .lock()
            .unwrap()
            .get(key)
            .is_some_and(|t| t.state == TaskState::Running)
    }
}

/// Everything the setup screen shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct SetupStatus {
    pub system: SystemReport,
    /// Odoo versions ready to use (their source is on this computer), newest first.
    pub versions: Vec<String>,
    /// The versions in use, with where their files are.
    pub sources: Vec<VersionInfo>,
    /// Odoo source found on the computer that is not used yet.
    pub found: Vec<FoundSource>,
    /// At least one AI assistant is installed and logged in.
    pub ai_ready: bool,
    /// Long steps still going or finished in this run of the app.
    pub tasks: HashMap<String, Task>,
}

fn task_key(edition: Edition, version: &str) -> String {
    match edition {
        Edition::Community => format!("community:{version}"),
        Edition::Enterprise => format!("enterprise:{version}"),
    }
}

/// Record a task's state and tell the UI.
fn update<R: Runtime>(app: &AppHandle<R>, setup: &SetupState, key: &str, task: Task) {
    setup
        .tasks
        .lock()
        .unwrap()
        .insert(key.to_string(), task.clone());
    let _ = app.emit(
        SETUP_EVENT,
        TaskUpdate {
            key: key.to_string(),
            task,
        },
    );
}

fn running(progress: Option<u8>, message: &str) -> Task {
    Task {
        state: TaskState::Running,
        progress,
        message: message.into(),
    }
}

fn finished(state: TaskState, message: impl Into<String>) -> Task {
    Task {
        state,
        progress: None,
        message: message.into(),
    }
}

#[tauri::command]
pub async fn setup_status(state: State<'_, AppState>) -> Result<SetupStatus, String> {
    let (setup, odoo, providers) = (
        state.setup.clone(),
        state.odoo.clone(),
        state.providers.clone(),
    );
    tauri::async_runtime::spawn_blocking(move || {
        let sources: Vec<VersionInfo> = odoo
            .downloaded_versions()
            .into_iter()
            .filter(|v| v.community != SourceState::Missing)
            .collect();
        let versions = sources.iter().map(|v| v.version.clone()).collect();
        let found = odoo.unused_sources(&setup.hooks.home);
        SetupStatus {
            system: system::report(&(setup.hooks.probe)()),
            versions,
            sources,
            found,
            ai_ready: providers().iter().any(|p| p.ready()),
            tasks: setup.tasks.lock().unwrap().clone(),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Use the Odoo source found on the computer where it is: nothing is downloaded and
/// the folders are never changed. Returns the versions newly available.
#[tauri::command]
pub async fn use_found_sources(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let (odoo, home) = (state.odoo.clone(), state.setup.hooks.home.clone());
    tauri::async_runtime::spawn_blocking(move || odoo.use_found_sources(&home))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

/// Use a folder the person picked (an odev worktree folder, or an `odoo` checkout):
/// its version is read from the code. Returns that version.
#[tauri::command]
pub fn use_source_folder(state: State<'_, AppState>, path: PathBuf) -> Result<String, String> {
    state
        .odoo
        .use_source_at(&path)
        .map_err(|e| format!("{e:#}"))
}

/// Whether the app's key reads Odoo's repositories (for downloading, in Settings).
#[tauri::command]
pub async fn github_status_cmd(state: State<'_, AppState>) -> Result<GithubStatus, String> {
    let (setup, odoo) = (state.setup.clone(), state.odoo.clone());
    tauri::async_runtime::spawn_blocking(move || (setup.hooks.github)(&odoo))
        .await
        .map_err(|e| e.to_string())
}

/// Install and start whatever the computer is missing, with one password prompt.
/// Resolves when done; progress also arrives as "tools" task events.
#[tauri::command]
pub async fn install_tools<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let setup = state.setup.clone();
    if setup.running("tools") {
        return Err("The tools are already being installed.".into());
    }
    let user = setup.hooks.user.clone();
    if !system::valid_user(&user) {
        return Err(format!("Cannot set up a database user for \"{user}\"."));
    }
    let fixes = system::plan(&(setup.hooks.probe)());
    if fixes.is_empty() {
        update(
            &app,
            &setup,
            "tools",
            finished(TaskState::Done, "Everything is installed."),
        );
        return Ok(());
    }
    update(
        &app,
        &setup,
        "tools",
        running(None, "Installing… (type your password in the window)"),
    );
    let script = system::script(&fixes, &user);
    let result = tauri::async_runtime::spawn_blocking(move || {
        (setup.hooks.elevate)(&script).and_then(|_| {
            // Check again: the script ran, but only a working machine counts.
            let report = system::report(&(setup.hooks.probe)());
            match report.requirements.iter().find(|r| !r.ok) {
                None => Ok(()),
                Some(r) => Err(r.problem.clone()),
            }
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    let setup = &state.setup;
    match result {
        Ok(()) => {
            update(
                &app,
                setup,
                "tools",
                finished(TaskState::Done, "Everything is installed."),
            );
            Ok(())
        }
        Err(e) => {
            update(&app, setup, "tools", finished(TaskState::Failed, e.clone()));
            Err(e)
        }
    }
}

/// Start downloading a version's Community or Enterprise source in the background.
/// After Community, Odoo's Python packages are prepared too, so the first spec is fast.
#[tauri::command]
pub fn download_odoo<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    version: String,
    edition: Edition,
) -> Result<(), String> {
    let key = task_key(edition, &version);
    let setup = state.setup.clone();
    if setup.running(&key) {
        return Ok(());
    }
    let odoo = state.odoo.clone();
    update(&app, &setup, &key, running(Some(0), "Downloading…"));
    std::thread::spawn(move || {
        let outcome = odoo.download_and_prepare(&version, edition, &mut |progress| {
            let message = match progress {
                Some(_) => "Downloading…",
                None => "Preparing Odoo… (a few minutes)",
            };
            update(&app, &setup, &key, running(progress, message));
        });
        let task = match outcome {
            Ok(()) => finished(TaskState::Done, "Ready."),
            Err(e) if e.is::<NoAccess>() => finished(TaskState::NoAccess, e.to_string()),
            Err(e) => finished(TaskState::Failed, format!("{e:#}")),
        };
        update(&app, &setup, &key, task);
    });
    Ok(())
}

/// Connect GitHub: make the app's own SSH key, then one browser sign-in (in a
/// terminal window) that allows adding keys and adds it to the person's account.
#[tauri::command]
pub fn github_connect(state: State<'_, AppState>) -> Result<(), String> {
    state.odoo.ensure_ssh_key().map_err(|e| format!("{e:#}"))?;
    let public = state.odoo.ssh_key_path().with_extension("pub");
    // Already signed in: only ask for the extra permission; then add the key
    // ("already in use" just means it was added before).
    let script = r#"echo "GitHub shows a one-time code below (like ABCD-1234)."
echo "Type it on the GitHub page that opens (github.com/login/device)."
echo "It is not sent by email: it is only shown here."
echo
if gh auth status --hostname github.com >/dev/null 2>&1; then
  gh auth refresh --hostname github.com --scopes admin:public_key
else
  gh auth login --hostname github.com --web --scopes admin:public_key
fi
gh ssh-key add "$1" --title "Odoo Dev Specs ($(hostname))" 2>&1 | grep -v "already in use" || true"#;
    (state.launch)(&[
        "sh".into(),
        "-c".into(),
        script.into(),
        "sh".into(),
        public.display().to_string(),
    ])
}

/// The Odoo versions on this computer, and where downloads go. Local only, so it
/// answers at once; the versions Odoo publishes come from `odoo_published_versions`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct OdooVersions {
    pub downloaded: Vec<VersionInfo>,
    /// The folder downloads go to, e.g. ~/odoo-versions (one folder per version).
    pub downloads_folder: String,
}

#[tauri::command]
pub async fn odoo_versions(state: State<'_, AppState>) -> Result<OdooVersions, String> {
    let odoo = state.odoo.clone();
    tauri::async_runtime::spawn_blocking(move || OdooVersions {
        downloaded: odoo.downloaded_versions(),
        downloads_folder: odoo.downloads_dir().display().to_string(),
    })
    .await
    .map_err(|e| e.to_string())
}

/// The versions Odoo publishes on GitHub (may take a few seconds, or fail offline).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct PublishedVersions {
    pub available: Vec<String>,
    pub latest_saas: Option<String>,
}

#[tauri::command]
pub async fn odoo_published_versions(
    state: State<'_, AppState>,
) -> Result<PublishedVersions, String> {
    let odoo = state.odoo.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let available = odoo.available_versions().map_err(|e| format!("{e:#}"))?;
        Ok(PublishedVersions {
            latest_saas: available.iter().find(|v| v.starts_with("saas-")).cloned(),
            available,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn remove_odoo_version(
    state: State<'_, AppState>,
    version: String,
) -> Result<(), String> {
    let odoo = state.odoo.clone();
    tauri::async_runtime::spawn_blocking(move || odoo.remove_version(&version))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

/// Update a version's source now (the weekly update, on demand).
#[tauri::command]
pub async fn update_odoo_version(
    state: State<'_, AppState>,
    version: String,
) -> Result<(), String> {
    let odoo = state.odoo.clone();
    tauri::async_runtime::spawn_blocking(move || odoo.update_if_stale(&version, Duration::ZERO))
        .await
        .map_err(|e| e.to_string())?
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

/// At start, quietly bring each version older than a week up to date.
pub fn start_weekly_updates(odoo: Arc<OdooRuntime>) {
    std::thread::spawn(move || {
        for (version, e) in odoo.update_stale_versions(WEEK) {
            eprintln!("could not update Odoo {version}: {e:#}");
        }
    });
}

/// Use a folder the person already has for a version's Community or Enterprise.
#[tauri::command]
pub fn use_odoo_folder(
    state: State<'_, AppState>,
    version: String,
    edition: Edition,
    path: PathBuf,
) -> Result<(), String> {
    state
        .odoo
        .use_folder(&version, edition, &path)
        .map_err(|e| format!("{e:#}"))
}

// --- The real machine --------------------------------------------------------------

/// Ubuntu's password window (`pkexec`) running the script as root.
fn run_as_root(script: &str) -> Result<String, String> {
    let out = Command::new("pkexec")
        .args(["/bin/bash", "-c", script])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| {
            "The password window (pkexec) is not available on this computer.".to_string()
        })?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    match out.status.code() {
        Some(0) => Ok(text),
        // pkexec: the window was closed or the password was wrong three times.
        Some(126) | Some(127) => {
            Err("The password was not given, so nothing was installed.".into())
        }
        _ => {
            let last: Vec<&str> = text.lines().rev().take(6).collect();
            let last: Vec<&str> = last.into_iter().rev().collect();
            Err(format!("Installing failed:\n{}", last.join("\n")))
        }
    }
}

fn github_status(odoo: &OdooRuntime) -> GithubStatus {
    let installed = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("gh").is_file()));
    // Without the app's key there is nothing to test yet.
    let access = if odoo.ssh_key_path().is_file() {
        odoo.github_access()
    } else {
        Default::default()
    };
    GithubStatus {
        installed,
        connected: access.community,
        enterprise: access.enterprise,
    }
}
