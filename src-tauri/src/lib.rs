mod commands;
mod diagram_window;

pub use commands::{register, AppState, ChatEventPayload, CHAT_EVENT};
pub use diagram_window::{DIAGRAM_DOCKED_EVENT, DIAGRAM_SESSION_EVENT, DIAGRAM_WINDOW};

use odev_core::pruning::{self, Outcome};
use specs_core::chat::ClaudeConfig;
use specs_core::locate::{find_executable, user_bin_dirs};
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

/// Claude Code on this machine, with the Odoo source as read-only context.
fn detect_claude() -> Option<ClaudeConfig> {
    let home = dirs::home_dir().unwrap_or_default();
    let path_var = std::env::var("PATH").ok();
    let program = find_executable("claude", path_var.as_deref(), &user_bin_dirs(&home))?;
    let read_dirs = pruning::default_config_path()
        .map(|cfg| odev_core::paths::odoo_source_dirs(&cfg))
        .unwrap_or_default();
    eprintln!("claude: {} (read-only: {read_dirs:?})", program.display());
    Some(ClaudeConfig {
        program,
        read_dirs,
        model: None,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    patch_odev_pruning();
    let state = AppState::new(Workspace::new(Workspace::default_root()), detect_claude());
    register(tauri::Builder::default(), state)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running Odoo Dev Specs");
}
