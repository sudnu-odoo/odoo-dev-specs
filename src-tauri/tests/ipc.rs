//! Commands called through Tauri's IPC layer (mock runtime), as the UI calls them.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use odoo_dev_specs_lib::{register, AppState, CHAT_EVENT, DIAGRAM_SESSION_EVENT, DIAGRAM_WINDOW};
use serde_json::{json, Value};
use specs_core::chat::ClaudeConfig;
use specs_core::workspace::Workspace;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, Manager, WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/specs-core/tests/fixtures/claude_read_tool.jsonl"
);

/// Fake `claude`, written once before any process is started (avoids "Text file busy").
/// Replays the fixture; `slow` in the session folder makes it hang until killed.
fn fake_claude() -> PathBuf {
    static FAKE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    FAKE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude");
        let script = format!(
            "#!/bin/sh\ncat >/dev/null\nif [ -f slow ]; then exec sleep 30; fi\ncat '{FIXTURE}'\n"
        );
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        (dir, path)
    })
    .1
    .clone()
}

struct Harness {
    app: App<MockRuntime>,
    window: WebviewWindow<MockRuntime>,
    root: PathBuf,
    _dir: TempDir,
}

fn harness(with_claude: bool) -> Harness {
    let claude = with_claude.then(|| ClaudeConfig {
        program: fake_claude(),
        read_dirs: vec![],
        model: None,
        diagram_tools: None,
        odev: None,
    });
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("OdooSpecs");
    let state = AppState::new(Workspace::new(&root), claude);
    let app = register(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    Harness {
        app,
        window,
        root,
        _dir: dir,
    }
}

impl Harness {
    fn call(&self, cmd: &str, args: Value) -> Result<Value, String> {
        let request = InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            // The app's own origin on Linux; anything else counts as remote and is denied.
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        get_ipc_response(&self.window, request)
            .map(|body| body.deserialize::<Value>().unwrap())
            .map_err(|e| {
                e.as_str()
                    .map(String::from)
                    .unwrap_or_else(|| e.to_string())
            })
    }

    fn new_session(&self) -> String {
        self.call("create_project", json!({"name": "Acme"}))
            .unwrap();
        let meta = self
            .call(
                "create_session",
                json!({"project": "Acme", "title": "Delivery"}),
            )
            .unwrap();
        meta["id"].as_str().unwrap().to_string()
    }

    /// Chat events of one turn, up to and including `finished`.
    fn events_until_finished(&self, start: impl FnOnce()) -> Vec<Value> {
        let (tx, rx) = mpsc::channel();
        self.app.listen_any(CHAT_EVENT, move |e| {
            let _ = tx.send(serde_json::from_str::<Value>(e.payload()).unwrap());
        });
        start();
        let mut events = Vec::new();
        loop {
            let payload = rx
                .recv_timeout(Duration::from_secs(10))
                .expect("no finished event");
            let done = payload["event"]["kind"] == "finished";
            events.push(payload);
            if done {
                return events;
            }
        }
    }
}

#[test]
fn project_and_session_lifecycle() {
    let h = harness(true);
    assert_eq!(h.call("list_projects", json!({})).unwrap(), json!([]));
    let id = h.new_session();

    let projects = h.call("list_projects", json!({})).unwrap();
    assert_eq!(projects[0]["name"], "Acme");
    assert_eq!(projects[0]["sessions"][0]["title"], "Delivery");

    let renamed = h
        .call(
            "rename_session",
            json!({"project": "Acme", "sessionId": id, "title": "Renamed"}),
        )
        .unwrap();
    assert_eq!(renamed["title"], "Renamed");

    let data = h
        .call("load_session", json!({"project": "Acme", "sessionId": id}))
        .unwrap();
    assert_eq!(data["meta"]["title"], "Renamed");
    assert_eq!(data["transcript"], json!([]));
    let scene: Value = serde_json::from_str(data["scene"].as_str().unwrap()).unwrap();
    assert_eq!(scene["type"], "excalidraw");

    h.call(
        "delete_session",
        json!({"project": "Acme", "sessionId": id}),
    )
    .unwrap();
    assert!(!h.root.join("Acme").join(&id).exists());
}

#[test]
fn errors_come_back_as_readable_messages() {
    let h = harness(true);
    h.call("create_project", json!({"name": "Acme"})).unwrap();
    let err = h
        .call("create_project", json!({"name": "Acme"}))
        .unwrap_err();
    assert!(err.contains("already exists"), "{err}");

    let id = h.new_session_in_existing();
    let err = h
        .call(
            "save_scene",
            json!({"project": "Acme", "sessionId": id, "scene": "nope"}),
        )
        .unwrap_err();
    assert!(err.contains("not valid JSON"), "{err}");
}

impl Harness {
    fn new_session_in_existing(&self) -> String {
        let meta = self
            .call("create_session", json!({"project": "Acme", "title": "t"}))
            .unwrap();
        meta["id"].as_str().unwrap().to_string()
    }
}

#[test]
fn scene_saves_round_trip() {
    let h = harness(true);
    let id = h.new_session();
    let scene =
        r#"{"type":"excalidraw","version":2,"elements":[{"id":"a"}],"appState":{},"files":{}}"#;
    h.call(
        "save_scene",
        json!({"project": "Acme", "sessionId": id, "scene": scene}),
    )
    .unwrap();
    let data = h
        .call("load_session", json!({"project": "Acme", "sessionId": id}))
        .unwrap();
    assert_eq!(data["scene"], scene);
}

#[test]
fn chat_turn_streams_events_and_persists_transcript() {
    let h = harness(true);
    let id = h.new_session();
    let events = h.events_until_finished(|| {
        h.call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "Spec it"}),
        )
        .unwrap();
    });

    assert!(events.iter().all(|e| e["sessionId"] == id.as_str()));
    let kinds: Vec<_> = events
        .iter()
        .map(|e| e["event"]["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["tool_started", "tool_finished", "text_delta", "finished"]
    );

    let data = h
        .call("load_session", json!({"project": "Acme", "sessionId": id}))
        .unwrap();
    let roles: Vec<_> = data["transcript"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["role"].clone())
        .collect();
    assert_eq!(roles, ["user", "tool", "assistant"]);
    assert_eq!(data["meta"]["started"], true);
}

#[test]
fn cancel_stops_the_turn_and_blocks_overlap_until_it_ends() {
    let h = harness(true);
    let id = h.new_session();
    fs::write(h.root.join("Acme").join(&id).join("slow"), "").unwrap();
    let send = || {
        h.call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "x"}),
        )
    };

    let events = h.events_until_finished(|| {
        send().unwrap();
        let err = send().unwrap_err();
        assert!(err.contains("still answering"), "{err}");
        h.call("cancel_turn", json!({"sessionId": id})).unwrap();
    });
    assert_eq!(
        events.last().unwrap()["event"],
        json!({"kind": "finished", "is_error": true, "message": "Stopped."})
    );
}

#[test]
fn missing_claude_is_reported() {
    let h = harness(false);
    let id = h.new_session();
    let err = h
        .call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "x"}),
        )
        .unwrap_err();
    assert!(err.contains("Claude Code is not installed"), "{err}");
}

#[test]
fn load_scene_returns_the_stored_diagram() {
    let h = harness(true);
    let id = h.new_session();
    let scene = r#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;
    h.call(
        "save_scene",
        json!({"project": "Acme", "sessionId": id, "scene": scene}),
    )
    .unwrap();
    assert_eq!(
        h.call("load_scene", json!({"project": "Acme", "sessionId": id}))
            .unwrap(),
        scene
    );
}

/// Events of `name` received while `act` runs.
fn collect_events(h: &Harness, name: &str, act: impl FnOnce()) -> Vec<Value> {
    let (tx, rx) = mpsc::channel();
    h.app.listen_any(name, move |e| {
        let _ = tx.send(serde_json::from_str::<Value>(e.payload()).unwrap());
    });
    act();
    rx.try_iter().collect()
}

#[test]
fn diagram_window_opens_once_and_follows_the_selected_session() {
    let h = harness(true);
    let id = h.new_session();
    let open = |title: &str| {
        h.call(
            "open_diagram_window",
            json!({"project": "Acme", "sessionId": id, "title": title}),
        )
        .unwrap();
    };

    // Window titles are not stored by Tauri's mock runtime, so they are checked in the real app.
    open("Delivery");
    assert!(h.app.get_webview_window(DIAGRAM_WINDOW).is_some());

    let events = collect_events(&h, DIAGRAM_SESSION_EVENT, || open("Renamed"));
    assert_eq!(h.app.webview_windows().len(), 2, "no second diagram window");
    assert_eq!(
        events,
        vec![json!({"project": "Acme", "sessionId": id, "title": "Renamed"})]
    );
}

/// The mock runtime drops a destroyed window without the `Destroyed` window event that
/// the real runtime sends, so "the main window is told" is verified in the real app.
#[test]
fn docking_destroys_the_diagram_window_and_is_idempotent() {
    let h = harness(true);
    let id = h.new_session();
    h.call(
        "open_diagram_window",
        json!({"project": "Acme", "sessionId": id, "title": "t"}),
    )
    .unwrap();

    h.call("dock_diagram_window", json!({})).unwrap();
    h.call("dock_diagram_window", json!({})).unwrap();
}
