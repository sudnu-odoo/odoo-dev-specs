use odev_core::pruning::{self, Outcome};

/// Write a file chosen by the user through the native save dialog.
/// The path always comes from the dialog, never from the AI.
#[tauri::command]
fn write_file(path: String, contents: Vec<u8>) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("Could not save {path}: {e}"))
}

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    patch_odev_pruning();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![write_file])
        .run(tauri::generate_context!())
        .expect("error while running Odoo Dev Specs");
}
