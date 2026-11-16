//! Tauri commands: thin wrappers over `specs-core`. Errors are returned as
//! plain sentences the UI can show to a business user.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use odev_core::doctor::{self, Probe, Report};
use serde::Serialize;
use specs_core::chat::{self, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::settings::{Settings, SettingsStore};
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::{ProjectInfo, SessionMeta, Workspace};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::oneshot;

use crate::diagram_window;

/// Name of the event carrying [`ChatEventPayload`] to the UI.
pub const CHAT_EVENT: &str = "chat-event";

pub struct AppState {
    pub workspace: Arc<Workspace>,
    /// `None` when Claude Code is not installed.
    pub claude: Option<ClaudeConfig>,
    /// Session id -> cancel switch of its running turn. The entry stays until the
    /// turn has really ended (the switch is taken when cancelling), so a new turn
    /// cannot start while the old process is still shutting down.
    pub running: Mutex<HashMap<String, Option<oneshot::Sender<()>>>>,
    /// Where user preferences are stored.
    pub settings: Arc<SettingsStore>,
    /// Gathers the machine state for the first-run check; swappable in tests.
    pub probe: Arc<dyn Fn() -> Probe + Send + Sync>,
}

impl AppState {
    pub fn new(
        workspace: Workspace,
        claude: Option<ClaudeConfig>,
        settings: SettingsStore,
    ) -> Self {
        Self {
            workspace: Arc::new(workspace),
            claude,
            running: Mutex::default(),
            settings: Arc::new(settings),
            probe: Arc::new(crate::environment::probe),
        }
    }

    /// Replace the environment probe (tests use a fixed machine state).
    pub fn with_probe(mut self, probe: impl Fn() -> Probe + Send + Sync + 'static) -> Self {
        self.probe = Arc::new(probe);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEventPayload {
    pub session_id: String,
    pub event: ChatEvent,
}

#[derive(Debug, Serialize)]
pub struct SessionData {
    pub meta: SessionMeta,
    /// The `.excalidraw` JSON, as stored.
    pub scene: String,
    pub transcript: Vec<TranscriptEntry>,
}

type CmdResult<T> = Result<T, String>;

fn user_error(e: anyhow::Error) -> String {
    e.to_string()
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> CmdResult<Vec<ProjectInfo>> {
    state.workspace.list().map_err(user_error)
}

#[tauri::command]
pub fn create_project(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    state.workspace.create_project(&name).map_err(user_error)
}

#[tauri::command]
pub fn create_session(
    state: State<'_, AppState>,
    project: String,
    title: String,
) -> CmdResult<SessionMeta> {
    state
        .workspace
        .create_session(&project, &title, "claude")
        .map_err(user_error)
}

#[tauri::command]
pub fn rename_session(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
    title: String,
) -> CmdResult<SessionMeta> {
    state
        .workspace
        .rename_session(&project, &session_id, &title)
        .map_err(user_error)
}

#[tauri::command]
pub fn delete_session(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
) -> CmdResult<()> {
    if state.running.lock().unwrap().contains_key(&session_id) {
        return Err("Stop the assistant before deleting this session.".into());
    }
    state
        .workspace
        .delete_session(&project, &session_id)
        .map_err(user_error)
}

#[tauri::command]
pub fn load_session(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
) -> CmdResult<SessionData> {
    let ws = &state.workspace;
    Ok(SessionData {
        meta: ws.session(&project, &session_id).map_err(user_error)?,
        scene: ws.read_scene(&project, &session_id).map_err(user_error)?,
        transcript: ws
            .read_transcript(&project, &session_id)
            .map_err(user_error)?,
    })
}

#[tauri::command]
pub fn load_scene(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
) -> CmdResult<String> {
    state
        .workspace
        .read_scene(&project, &session_id)
        .map_err(user_error)
}

#[tauri::command]
pub fn save_scene(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
    scene: String,
) -> CmdResult<()> {
    state
        .workspace
        .write_scene(&project, &session_id, &scene)
        .map_err(user_error)
}

/// Start a turn in the background. Progress arrives as [`CHAT_EVENT`] events,
/// always ending with a `finished` event.
#[tauri::command]
pub fn send_message<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    project: String,
    session_id: String,
    text: String,
) -> CmdResult<()> {
    let Some(mut claude) = state.claude.clone() else {
        return Err(
            "Claude Code is not installed. Install it and run `claude` once to log in.".into(),
        );
    };
    // Apply the latest settings, so changing provider or demo data takes effect
    // without restarting the app.
    let current = state.settings.load();
    claude.provider = current.provider;
    claude.demo_data = current.demo_data;
    if text.trim().is_empty() {
        return Err("The message is empty.".into());
    }
    let (cancel_tx, cancel_rx) = oneshot::channel();
    {
        let mut running = state.running.lock().unwrap();
        if running.contains_key(&session_id) {
            return Err("The assistant is still answering in this session.".into());
        }
        running.insert(session_id.clone(), Some(cancel_tx));
    }

    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn(async move {
        let emit = |event: ChatEvent| {
            let payload = ChatEventPayload {
                session_id: session_id.clone(),
                event,
            };
            let _ = app.emit(CHAT_EVENT, payload);
        };
        let result = chat::send_message(
            &workspace,
            &project,
            &session_id,
            &text,
            &claude,
            emit,
            cancel_rx,
        )
        .await;
        // Free the session before announcing the end, so the UI can send right away.
        app.state::<AppState>()
            .running
            .lock()
            .unwrap()
            .remove(&session_id);
        let last = result.unwrap_or_else(|e| ChatEvent::Finished {
            is_error: true,
            message: Some(e.to_string()),
        });
        emit(last);
    });
    Ok(())
}

#[tauri::command]
pub fn cancel_turn(state: State<'_, AppState>, session_id: String) {
    if let Some(cancel) = state
        .running
        .lock()
        .unwrap()
        .get_mut(&session_id)
        .and_then(Option::take)
    {
        let _ = cancel.send(());
    }
}

/// Write a file chosen by the user through the native save dialog.
/// The path always comes from the dialog, never from the AI.
#[tauri::command]
pub fn write_file(path: String, contents: Vec<u8>) -> CmdResult<()> {
    std::fs::write(&path, contents).map_err(|e| format!("Could not save {path}: {e}"))
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.load()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    state.settings.save(&settings).map_err(user_error)
}

/// The first-run / Settings environment report (odev, PostgreSQL, source, Chrome, AI).
#[tauri::command]
pub fn check_environment(state: State<'_, AppState>) -> Report {
    doctor::check(&(state.probe)())
}

/// Register state and every command on a builder (shared by the app and tests).
pub fn register<R: Runtime>(builder: tauri::Builder<R>, state: AppState) -> tauri::Builder<R> {
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            list_projects,
            create_project,
            create_session,
            rename_session,
            delete_session,
            load_session,
            load_scene,
            save_scene,
            send_message,
            cancel_turn,
            write_file,
            get_settings,
            save_settings,
            check_environment,
            diagram_window::open_diagram_window,
            diagram_window::dock_diagram_window,
        ])
        .on_window_event(diagram_window::on_window_event)
}
