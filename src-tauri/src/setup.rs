//! The first-run setup: what is missing on this computer, installing it with one
//! password prompt, downloading the Odoo source in the background (progress sent as
//! events), signing in to GitHub for Enterprise, and using folders people already have.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use odoo_runtime::runtime::{Edition, NoAccess, SourceState};
use odoo_runtime::system::{self, SystemProbe, SystemReport};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};

use crate::commands::AppState;

/// Name of the event carrying a [`TaskUpdate`] to the UI.
pub const SETUP_EVENT: &str = "setup-progress";

/// Runs the fix script as root; returns its output, or a sentence for the user.
pub type Elevate = dyn Fn(&str) -> Result<String, String> + Send + Sync;

/// Is GitHub's tool there, and is someone signed in with it?
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubStatus {
    pub installed: bool,
    pub signed_in: bool,
}

/// How the setup reaches the machine; swappable in tests.
pub struct SetupHooks {
    pub probe: Arc<dyn Fn() -> SystemProbe + Send + Sync>,
    pub elevate: Arc<Elevate>,
    pub github: Arc<dyn Fn() -> GithubStatus + Send + Sync>,
    /// The person's Linux user name, for their database user.
    pub user: String,
}

impl Default for SetupHooks {
    fn default() -> Self {
        Self {
            probe: Arc::new(system::probe),
            elevate: Arc::new(run_as_root),
            github: Arc::new(github_status),
            user: std::env::var("USER").unwrap_or_default(),
        }
    }
}

/// A long setup step, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// "running", "done", "failed" or "noAccess".
    pub state: &'static str,
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
            .is_some_and(|t| t.state == "running")
    }
}

/// Everything the setup screen shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub version: String,
    pub system: SystemReport,
    pub community: SourceState,
    pub enterprise: SourceState,
    pub github: GithubStatus,
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
        state: "running",
        progress,
        message: message.into(),
    }
}

fn finished(state: &'static str, message: impl Into<String>) -> Task {
    Task {
        state,
        progress: None,
        message: message.into(),
    }
}

#[tauri::command]
pub async fn setup_status(
    state: State<'_, AppState>,
    version: String,
) -> Result<SetupStatus, String> {
    let (setup, odoo, providers) = (
        state.setup.clone(),
        state.odoo.clone(),
        state.providers.clone(),
    );
    tauri::async_runtime::spawn_blocking(move || SetupStatus {
        system: system::report(&(setup.hooks.probe)()),
        community: odoo.source_state(&version, Edition::Community),
        enterprise: odoo.source_state(&version, Edition::Enterprise),
        github: (setup.hooks.github)(),
        ai_ready: providers().iter().any(|p| p.ready()),
        tasks: setup.tasks.lock().unwrap().clone(),
        version,
    })
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
            finished("done", "Everything is installed."),
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
                finished("done", "Everything is installed."),
            );
            Ok(())
        }
        Err(e) => {
            update(&app, setup, "tools", finished("failed", e.clone()));
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
        let mut report = |p: u8| update(&app, &setup, &key, running(Some(p), "Downloading…"));
        let outcome = odoo
            .download_source(&version, edition, &mut report)
            .and_then(|()| {
                if edition == Edition::Community {
                    update(
                        &app,
                        &setup,
                        &key,
                        running(None, "Preparing Odoo… (a few minutes)"),
                    );
                    odoo.ensure_venv(&version).map(|_| ())
                } else {
                    Ok(())
                }
            });
        let task = match outcome {
            Ok(()) => finished("done", "Ready."),
            Err(e) if e.is::<NoAccess>() => finished("noAccess", e.to_string()),
            Err(e) => finished("failed", format!("{e:#}")),
        };
        update(&app, &setup, &key, task);
    });
    Ok(())
}

/// Open GitHub's sign-in (browser with a short code) in a terminal window; Git then
/// uses that sign-in to download Enterprise.
#[tauri::command]
pub fn github_sign_in(state: State<'_, AppState>) -> Result<(), String> {
    (state.launch)(&[
        "sh".into(),
        "-c".into(),
        "gh auth login --hostname github.com --git-protocol https --web && gh auth setup-git"
            .into(),
    ])
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

fn github_status() -> GithubStatus {
    let installed = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("gh").is_file()));
    let signed_in = installed
        && Command::new("gh")
            .args(["auth", "status", "--hostname", "github.com"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
    GithubStatus {
        installed,
        signed_in,
    }
}
