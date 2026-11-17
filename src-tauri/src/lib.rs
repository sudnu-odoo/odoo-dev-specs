mod commands;
mod diagram_window;
mod environment;

pub use commands::{register, AppState, ChatEventPayload, CHAT_EVENT};
pub use diagram_window::{DIAGRAM_DOCKED_EVENT, DIAGRAM_SESSION_EVENT, DIAGRAM_WINDOW};
pub use environment::provider_status;

use odoo_runtime::runtime::{Runtime, Tools};
use specs_core::chat::ClaudeConfig;
use specs_core::locate::{find_executable, user_bin_dirs};
use specs_core::settings::SettingsStore;
use specs_core::workspace::Workspace;

/// The AI assistants on this machine, with the Odoo source as read-only context.
fn detect_claude(settings: &specs_core::settings::Settings) -> Option<ClaudeConfig> {
    let home = dirs::home_dir().unwrap_or_default();
    let path_var = std::env::var("PATH").ok();
    let bins = user_bin_dirs(&home);
    let program = find_executable("claude", path_var.as_deref(), &bins)?;
    let diagram_tools = sibling_program("specs-mcp");
    let runtime_dir = Runtime::default_root();
    eprintln!(
        "claude: {} (diagram tools: {diagram_tools:?}, odoo runtime: {})",
        program.display(),
        runtime_dir.display()
    );
    Some(ClaudeConfig {
        program,
        provider: settings.provider,
        // Filled per message from the downloaded sources (see send_message).
        read_dirs: vec![],
        model: None,
        effort: None,
        diagram_tools,
        runtime_dir: Some(runtime_dir),
        demo_data: settings.demo_data,
    })
}

/// Odoo servers started for screenshots keep running between chat turns; stop
/// them when the app closes.
fn stop_odoo_servers() {
    if let Err(e) = Runtime::new(Runtime::default_root(), Tools::default()).stop_all() {
        eprintln!("could not stop the local Odoo servers: {e:#}");
    }
}

/// A helper program shipped next to the app's own executable.
fn sibling_program(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::current_exe().ok()?.parent()?.join(name);
    path.is_file().then_some(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = SettingsStore::new(SettingsStore::default_path());
    let claude = detect_claude(&settings.load());
    let state = AppState::new(Workspace::new(Workspace::default_root()), claude, settings);
    register(tauri::Builder::default(), state)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .build(tauri::generate_context!())
        .expect("error while starting Odoo Dev Specs")
        .run(move |_, event| {
            if let tauri::RunEvent::Exit = event {
                stop_odoo_servers();
            }
        });
}
