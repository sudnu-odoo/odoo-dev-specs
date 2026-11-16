//! The diagram can live in its own window ("undocked") so a user can chat in one
//! window and edit the diagram in another. There is at most one diagram window;
//! it follows the session selected in the main window.

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent,
};

pub const MAIN_WINDOW: &str = "main";
pub const DIAGRAM_WINDOW: &str = "diagram";
/// Sent to the diagram window: show this session (payload: [`DiagramTarget`]).
pub const DIAGRAM_SESSION_EVENT: &str = "diagram-session";
/// Sent when the diagram window is gone, so the main window shows the diagram again.
pub const DIAGRAM_DOCKED_EVENT: &str = "diagram-docked";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramTarget {
    pub project: String,
    pub session_id: String,
    pub title: String,
}

fn window_title(title: &str) -> String {
    format!("{title} — Diagram")
}

/// Open the diagram window on a session, or point the existing one at it.
#[tauri::command]
pub fn open_diagram_window<R: Runtime>(
    app: AppHandle<R>,
    project: String,
    session_id: String,
    title: String,
) -> Result<(), String> {
    let target = DiagramTarget {
        project,
        session_id,
        title,
    };
    if let Some(window) = app.get_webview_window(DIAGRAM_WINDOW) {
        window
            .set_title(&window_title(&target.title))
            .map_err(|e| e.to_string())?;
        let _ = window.set_focus();
        return app
            .emit_to(DIAGRAM_WINDOW, DIAGRAM_SESSION_EVENT, &target)
            .map_err(|e| e.to_string());
    }
    // The page reads its first session from this global before React starts.
    let init = format!(
        "window.__DIAGRAM_SESSION__ = {};",
        serde_json::to_string(&target).map_err(|e| e.to_string())?
    );
    WebviewWindowBuilder::new(&app, DIAGRAM_WINDOW, WebviewUrl::App("index.html".into()))
        .title(window_title(&target.title))
        .inner_size(1100.0, 800.0)
        .min_inner_size(600.0, 400.0)
        .initialization_script(&init)
        .build()
        .map(|_| ())
        .map_err(|e| format!("Could not open the diagram window: {e}"))
}

/// Close the diagram window; the main window shows the diagram again.
#[tauri::command]
pub fn dock_diagram_window<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(DIAGRAM_WINDOW) {
        window.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Keeps the two windows consistent however a window gets closed.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if !matches!(event, WindowEvent::Destroyed) {
        return;
    }
    let app = window.app_handle();
    match window.label() {
        DIAGRAM_WINDOW => {
            let _ = app.emit(DIAGRAM_DOCKED_EVENT, ());
        }
        // Closing the main window ends the app, including an undocked diagram.
        MAIN_WINDOW => {
            if let Some(diagram) = app.get_webview_window(DIAGRAM_WINDOW) {
                let _ = diagram.destroy();
            }
        }
        _ => {}
    }
}
