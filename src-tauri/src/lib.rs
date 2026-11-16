mod commands;
mod diagram_window;
mod environment;

pub use commands::{register, AppState, ChatEventPayload, CHAT_EVENT};
pub use diagram_window::{DIAGRAM_DOCKED_EVENT, DIAGRAM_SESSION_EVENT, DIAGRAM_WINDOW};

use odev_core::pruning::{self, Outcome};
use odev_core::server::Odev;
use specs_core::chat::ClaudeConfig;
use specs_core::locate::{find_executable, user_bin_dirs};
use specs_core::settings::SettingsStore;
use specs_core::workspace::Workspace;

/// Keep odev from prompting about old databases (it would block headless runs).
fn patch_odev_pruning() {
    match pruning::ensure_default() {
        Ok(Outcome::Updated { old, new }) => {
            eprintln!(
                "odev pruning date updated: {} -> {new}",
                old.as_deref().unwrap_or("<missing>")
            )
        }
        Ok(Outcome::AlreadyOk(date)) => eprintln!("odev pruning date ok ({date})"),
        Ok(Outcome::NoConfig) => eprintln!("odev config not found; is odev installed?"),
        Err(e) => eprintln!("could not patch odev config: {e:#}"),
    }
}

/// The AI assistants on this machine, with the Odoo source as read-only context.
/// Requires Claude Code (the default); Gemini is added when installed.
fn detect_claude(settings: &specs_core::settings::Settings) -> Option<ClaudeConfig> {
    let home = dirs::home_dir().unwrap_or_default();
    let path_var = std::env::var("PATH").ok();
    let bins = user_bin_dirs(&home);
    let program = find_executable("claude", path_var.as_deref(), &bins)?;
    let gemini_program = find_executable("gemini", path_var.as_deref(), &bins);
    let read_dirs = pruning::default_config_path()
        .map(|cfg| odev_core::paths::odoo_source_dirs(&cfg))
        .unwrap_or_default();
    let diagram_tools = sibling_program("specs-mcp");
    let odev = find_executable("odev", path_var.as_deref(), &bins);
    eprintln!(
        "claude: {} (gemini: {gemini_program:?}, read-only: {read_dirs:?}, diagram tools: {diagram_tools:?}, odev: {odev:?})",
        program.display()
    );
    Some(ClaudeConfig {
        program,
        gemini_program,
        provider: settings.provider,
        read_dirs,
        model: None,
        diagram_tools,
        odev,
        demo_data: settings.demo_data,
    })
}

/// Odoo servers started for screenshots keep running between chat turns; stop
/// them when the app closes.
fn stop_odoo_servers(odev: Option<&std::path::Path>) {
    if let Some(odev) = odev {
        if let Err(e) = Odev::new(odev, Odev::default_state_dir()).stop_all() {
            eprintln!("could not stop the local Odoo servers: {e:#}");
        }
    }
}

/// A helper program shipped next to the app's own executable.
fn sibling_program(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::current_exe().ok()?.parent()?.join(name);
    path.is_file().then_some(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    patch_odev_pruning();
    let settings = SettingsStore::new(SettingsStore::default_path());
    let claude = detect_claude(&settings.load());
    let odev = claude.as_ref().and_then(|c| c.odev.clone());
    let state = AppState::new(Workspace::new(Workspace::default_root()), claude, settings);
    register(tauri::Builder::default(), state)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .build(tauri::generate_context!())
        .expect("error while starting Odoo Dev Specs")
        .run(move |_, event| {
            if let tauri::RunEvent::Exit = event {
                stop_odoo_servers(odev.as_deref());
            }
        });
}
