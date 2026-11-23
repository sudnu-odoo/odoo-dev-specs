//! Tauri commands: thin wrappers over `specs-core`. Errors are returned as
//! plain sentences the UI can show to a business user.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use odoo_runtime::doctor::{self, Probe, Report};
use odoo_runtime::runtime::{DatabaseInfo, NewDatabase, Runtime as OdooRuntime, Tools};
use serde::Serialize;
use specs_core::chat::{self, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::models;
use specs_core::providers::{self, ProviderStatus};
use specs_core::settings::{Provider, Settings, SettingsStore};
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::{ProjectInfo, SessionMeta, Workspace};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::oneshot;

use crate::diagram_window;

/// Name of the event carrying [`ChatEventPayload`] to the UI.
pub const CHAT_EVENT: &str = "chat-event";

/// Opens a command for the user to follow (a login), or says why it could not.
pub type Launcher = dyn Fn(&[String]) -> Result<(), String> + Send + Sync;

pub struct AppState {
    pub workspace: Arc<Workspace>,
    /// `None` when Claude Code is not installed.
    pub claude: Option<ClaudeConfig>,
    /// Session id -> cancel switch of its running turn. The entry stays until the
    /// turn has really ended (the switch is taken when cancelling), so a new turn
    /// cannot start while the old process is still shutting down.
    pub running: Mutex<HashMap<String, Option<oneshot::Sender<()>>>>,
    /// Readiness of every provider; asked each time, so logging in while the app
    /// runs unlocks a provider without a restart. Swappable in tests.
    pub providers: Arc<dyn Fn() -> Vec<ProviderStatus> + Send + Sync>,
    /// Opens a login command for the user (a terminal window). Swappable in tests.
    pub launch: Arc<Launcher>,
    /// Where user preferences are stored.
    pub settings: Arc<SettingsStore>,
    /// Gathers the machine state for the first-run check; swappable in tests.
    pub probe: Arc<dyn Fn() -> Probe + Send + Sync>,
    /// The local Odoo databases the user manages; swappable in tests.
    pub odoo: Arc<OdooRuntime>,
    /// The first-run setup: checks, installs and downloads.
    pub setup: Arc<crate::setup::SetupState>,
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
            providers: Arc::new(crate::environment::provider_status),
            launch: Arc::new(crate::environment::open_in_terminal),
            settings: Arc::new(settings),
            probe: Arc::new(crate::environment::probe),
            odoo: Arc::new(OdooRuntime::for_app(
                OdooRuntime::default_root(),
                Tools::default(),
            )),
            setup: Arc::default(),
        }
    }

    /// Use another runtime folder and tools (tests).
    pub fn with_odoo(mut self, runtime: OdooRuntime) -> Self {
        self.odoo = Arc::new(runtime);
        self
    }

    /// Replace how the setup reaches the machine (tests).
    pub fn with_setup(mut self, hooks: crate::setup::SetupHooks) -> Self {
        self.setup = Arc::new(crate::setup::SetupState::new(hooks));
        self
    }

    /// Replace how provider readiness is checked (tests).
    pub fn with_providers(
        mut self,
        status: impl Fn() -> Vec<ProviderStatus> + Send + Sync + 'static,
    ) -> Self {
        self.providers = Arc::new(status);
        self
    }

    /// Replace how login commands are opened (tests record them instead).
    pub fn with_launcher(
        mut self,
        launch: impl Fn(&[String]) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.launch = Arc::new(launch);
        self
    }

    /// Replace the environment probe (tests use a fixed machine state).
    pub fn with_probe(mut self, probe: impl Fn() -> Probe + Send + Sync + 'static) -> Self {
        self.probe = Arc::new(probe);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct ChatEventPayload {
    pub session_id: String,
    pub event: ChatEvent,
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export)]
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
    model: Option<String>,
    effort: Option<String>,
) -> CmdResult<()> {
    let Some(base) = &state.claude else {
        return Err("The assistant is not set up.".into());
    };
    let meta = state
        .workspace
        .session(&project, &session_id)
        .map_err(user_error)?;
    let turn = specs_core::turn::prepare_turn(
        base,
        &state.settings.load(),
        &(state.providers)(),
        specs_core::turn::TurnRequest {
            session: &meta,
            text: &text,
            model,
            effort,
        },
    )
    .map_err(user_error)?;
    if let Some(provider) = turn.new_provider {
        state
            .workspace
            .set_provider(&project, &session_id, provider.id())
            .map_err(user_error)?;
    }
    let claude = turn.config;
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

/// Sessions whose assistant is answering right now, so the UI can offer Stop when
/// it opens one of them (also after the UI reloaded).
#[tauri::command]
pub fn running_sessions(state: State<'_, AppState>) -> Vec<String> {
    let mut ids: Vec<String> = state.running.lock().unwrap().keys().cloned().collect();
    ids.sort();
    ids
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

/// The models of every provider, for the chat's model picker. A provider that is not
/// ready (not installed, not logged in) is listed with its models locked.
#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> CmdResult<Vec<models::Model>> {
    let statuses = provider_statuses(&state).await?;
    Ok(models::all_models(|provider| {
        statuses.iter().any(|s| s.provider == provider && s.ready())
    }))
}

/// Whether each provider is installed and logged in (Settings and the model menu).
#[tauri::command]
pub async fn provider_status(state: State<'_, AppState>) -> CmdResult<Vec<ProviderStatus>> {
    provider_statuses(&state).await
}

/// Checking runs the CLIs (up to a second or two), so keep it off the UI thread.
async fn provider_statuses(state: &AppState) -> CmdResult<Vec<ProviderStatus>> {
    let check = state.providers.clone();
    tauri::async_runtime::spawn_blocking(move || check())
        .await
        .map_err(|e| e.to_string())
}

/// Open the provider's login in a terminal window. The UI re-checks the status
/// when the user comes back to the app.
#[tauri::command]
pub async fn login_provider(state: State<'_, AppState>, provider: Provider) -> CmdResult<()> {
    let statuses = provider_statuses(&state).await?;
    let status = statuses
        .iter()
        .find(|s| s.provider == provider)
        .ok_or("Unknown assistant.")?;
    let program = status
        .program
        .as_ref()
        .ok_or_else(|| format!("{} is not installed on this computer.", status.name))?;
    (state.launch)(&providers::login_command(provider, program))
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    state.settings.save(&settings).map_err(user_error)
}

/// The first-run / Settings environment report (Python, git, PostgreSQL, source, Chrome, AI).
#[tauri::command]
pub fn check_environment(state: State<'_, AppState>) -> Report {
    doctor::check(&(state.probe)())
}

// --- Local Odoo databases ---------------------------------------------------------

/// Run a database action off the UI thread; creating one can take minutes.
async fn with_odoo<T: Send + 'static>(
    state: &AppState,
    action: impl FnOnce(&OdooRuntime) -> anyhow::Result<T> + Send + 'static,
) -> CmdResult<T> {
    let odoo = state.odoo.clone();
    tauri::async_runtime::spawn_blocking(move || action(&odoo))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn list_databases(state: State<'_, AppState>) -> CmdResult<Vec<DatabaseInfo>> {
    with_odoo(&state, |o| o.list_databases()).await
}

#[tauri::command]
pub async fn create_database(
    state: State<'_, AppState>,
    database: NewDatabase,
) -> CmdResult<DatabaseInfo> {
    with_odoo(&state, move |o| o.create_database(&database)).await
}

/// What the "New database" form starts with for a version.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct DatabaseForm {
    /// The next free name, e.g. "db-19-3".
    pub suggested_label: String,
    /// This computer has the enterprise source for the version.
    pub enterprise_available: bool,
}

#[tauri::command]
pub async fn database_form(state: State<'_, AppState>, version: String) -> CmdResult<DatabaseForm> {
    with_odoo(&state, move |o| {
        Ok(DatabaseForm {
            suggested_label: o.suggested_label(&version)?,
            enterprise_available: o.enterprise_available(&version),
        })
    })
    .await
}

#[tauri::command]
pub async fn rename_database(
    state: State<'_, AppState>,
    name: String,
    label: String,
) -> CmdResult<()> {
    with_odoo(&state, move |o| o.rename_database(&name, &label)).await
}

#[tauri::command]
pub async fn duplicate_database(
    state: State<'_, AppState>,
    name: String,
    label: String,
) -> CmdResult<DatabaseInfo> {
    with_odoo(&state, move |o| o.duplicate_database(&name, &label)).await
}

#[tauri::command]
pub async fn reset_database(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    with_odoo(&state, move |o| o.reset_database(&name)).await
}

#[tauri::command]
pub async fn delete_database(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    with_odoo(&state, move |o| o.delete_database(&name)).await
}

/// Start the database's Odoo (if needed) and return the address to open.
#[tauri::command]
pub async fn start_database(state: State<'_, AppState>, name: String) -> CmdResult<String> {
    with_odoo(&state, move |o| o.open_database(&name)).await
}

#[tauri::command]
pub async fn stop_database(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    with_odoo(&state, move |o| o.stop_database(&name)).await
}

/// Set where the client's Odoo runs for this session ("online" or "sh").
#[tauri::command]
pub fn set_session_hosting(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
    hosting: Option<String>,
) -> CmdResult<()> {
    state
        .workspace
        .set_hosting(&project, &session_id, hosting.as_deref())
        .map(|_| ())
        .map_err(user_error)
}

/// Set the client's Odoo version for this session (e.g. "19.0", "saas-19.2").
#[tauri::command]
pub fn set_session_version(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
    version: Option<String>,
) -> CmdResult<()> {
    state
        .workspace
        .set_version(&project, &session_id, version.as_deref())
        .map(|_| ())
        .map_err(user_error)
}

/// Choose the database a session's screenshots use (`None` = Auto).
#[tauri::command]
pub fn set_session_database(
    state: State<'_, AppState>,
    project: String,
    session_id: String,
    database: Option<String>,
) -> CmdResult<()> {
    if state.running.lock().unwrap().contains_key(&session_id) {
        return Err("Wait until the assistant has finished to change the database.".into());
    }
    state
        .workspace
        .set_database(&project, &session_id, database.as_deref())
        .map_err(user_error)
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
            running_sessions,
            write_file,
            get_settings,
            save_settings,
            check_environment,
            list_models,
            provider_status,
            login_provider,
            list_databases,
            database_form,
            create_database,
            rename_database,
            duplicate_database,
            reset_database,
            delete_database,
            start_database,
            stop_database,
            set_session_database,
            set_session_hosting,
            set_session_version,
            crate::update::check_update,
            crate::update::install_update,
            crate::setup::setup_status,
            crate::setup::install_tools,
            crate::setup::download_odoo,
            crate::setup::github_connect,
            crate::setup::use_found_sources,
            crate::setup::use_source_folder,
            crate::setup::github_status_cmd,
            crate::setup::odoo_versions,
            crate::setup::odoo_published_versions,
            crate::setup::remove_odoo_version,
            crate::setup::update_odoo_version,
            crate::setup::use_odoo_folder,
            diagram_window::open_diagram_window,
            diagram_window::dock_diagram_window,
        ])
        .on_window_event(diagram_window::on_window_event)
}
