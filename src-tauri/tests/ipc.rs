//! Commands called through Tauri's IPC layer (mock runtime), as the UI calls them.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use odoo_dev_specs_lib::{register, AppState, CHAT_EVENT, DIAGRAM_SESSION_EVENT, DIAGRAM_WINDOW};
use odoo_dev_specs_lib::{GithubStatus, SetupHooks};
use odoo_runtime::doctor::Probe;
use odoo_runtime::runtime::{Runtime as OdooRuntime, Tools};
use odoo_runtime::system::SystemProbe;
use serde_json::{json, Value};
use specs_core::chat::ClaudeConfig;
use specs_core::providers::ProviderStatus;
use specs_core::settings::{Provider, SettingsStore};
use specs_core::workspace::Workspace;

/// A fully healthy machine, so `check_environment` reports ready unless a test says otherwise.
fn test_probe() -> Probe {
    Probe {
        python: Some("/bin/python3".into()),
        git: Some("/bin/git".into()),
        postgres_ready: true,
        postgres_client: true,
        odoo_sources: vec!["/src/odoo".into(), "/src/enterprise".into()],
        chrome: Some("/bin/chrome".into()),
        claude: Some("/bin/claude".into()),
        antigravity: Some("/bin/agy".into()),
    }
}
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

const AGY_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/specs-core/tests/fixtures/agy_read_tool.jsonl"
);

/// Fake `agy`, replaying a recorded Antigravity run.
fn fake_agy() -> PathBuf {
    static FAKE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    FAKE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agy");
        fs::write(&path, format!("#!/bin/sh\ncat '{AGY_FIXTURE}'\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        (dir, path)
    })
    .1
    .clone()
}

struct Harness {
    /// Login commands the app asked to open (instead of opening a terminal).
    launched: Arc<Mutex<Vec<Vec<String>>>>,
    /// The fake computer the setup checks; starts as a fresh Ubuntu.
    machine: Arc<Mutex<SystemProbe>>,
    /// Scripts the setup ran as root (instead of asking for a password).
    scripts: Arc<Mutex<Vec<String>>>,
    /// What the assistants' check answers, and how many times it ran.
    assistants: Arc<Mutex<Vec<ProviderStatus>>>,
    assistant_checks: Arc<std::sync::atomic::AtomicUsize>,
    app: App<MockRuntime>,
    window: WebviewWindow<MockRuntime>,
    root: PathBuf,
    _dir: TempDir,
}

fn harness(with_claude: bool) -> Harness {
    harness_with(with_claude, None)
}

fn status(provider: Provider, program: Option<PathBuf>, logged_in: bool) -> ProviderStatus {
    ProviderStatus {
        provider,
        name: provider.display_name().into(),
        installed: program.is_some(),
        logged_in,
        program,
    }
}

/// `antigravity`: the `agy` the app should find, when it is installed and logged in.
/// Without it, agy counts as installed at `/bin/agy` but not logged in.
fn harness_with(with_claude: bool, antigravity: Option<PathBuf>) -> Harness {
    let statuses = Arc::new(Mutex::new(vec![
        status(Provider::Claude, with_claude.then(fake_claude), with_claude),
        match antigravity {
            Some(p) => status(Provider::Antigravity, Some(p), true),
            None => status(Provider::Antigravity, Some("/bin/agy".into()), false),
        },
    ]));
    let assistant_checks: Arc<std::sync::atomic::AtomicUsize> = Arc::default();
    let (answers, counter) = (statuses.clone(), assistant_checks.clone());
    let launched: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let record = launched.clone();
    // The shared assistant setup always exists; which CLI is installed is in `statuses`.
    let claude = Some(ClaudeConfig {
        program: fake_claude(),
        provider: specs_core::settings::Provider::Claude,
        read_dirs: vec![],
        model: None,
        effort: None,
        diagram_tools: None,
        runtime_dir: None,
        demo_data: true,
    });
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("OdooSpecs");
    let settings = SettingsStore::new(dir.path().join("settings.json"));
    // PostgreSQL with one database of ours and one that is not.
    let psql = dir.path().join("psql");
    fs::write(
        &psql,
        "#!/bin/sh\nprintf 'specs_19_0_sale_demo|5242880\\nother_db|1\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&psql, fs::Permissions::from_mode(0o755)).unwrap();
    // git: the shared repository, a fetch GitHub refuses while `deny` exists, and
    // a worktree that is a quick fake checkout.
    let git = dir.path().join("git");
    fs::write(
        &git,
        format!(
            r#"#!/bin/sh
case "$*" in
  *" fetch "*|*ls-remote*)
    [ -f '{d}/deny' ] && {{ echo 'ERROR: Repository not found.' >&2; echo 'fatal: Could not read from remote repository.' >&2; exit 128; }}
    case "$*" in *ls-remote*) printf 'a\trefs/heads/18.0\na\trefs/heads/19.0\na\trefs/heads/saas-19.1\na\trefs/heads/saas-19.2\n';; esac
    printf 'Receiving objects:  40%% (4/10)\r' >&2; exit 0;;
  *"worktree add"*)
    prev=""; for a in "$@"; do [ "$prev" = "--force" ] && mkdir -p "$a/web_enterprise"; prev="$a"; done; exit 0;;
esac
exit 0
"#,
            d = dir.path().display()
        ),
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let odoo = OdooRuntime::new(
        dir.path().join("runtime"),
        Tools {
            psql,
            git,
            ..Tools::default()
        },
    )
    // Never the person's real ~/odoo-versions.
    .with_downloads_dir(dir.path().join("odoo-versions"));
    let machine = Arc::new(Mutex::new(SystemProbe {
        python: Some((3, 12)),
        ..SystemProbe::default()
    }));
    let scripts: Arc<Mutex<Vec<String>>> = Arc::default();
    let (probe_machine, fix_machine, ran) = (machine.clone(), machine.clone(), scripts.clone());
    let fail_install = dir.path().join("fail-install");
    // A home where tests put Odoo checkouts, like odev's worktrees.
    let home = dir.path().join("home");
    let hooks = SetupHooks {
        probe: Arc::new(move || probe_machine.lock().unwrap().clone()),
        elevate: Arc::new(move |script: &str| {
            ran.lock().unwrap().push(script.to_string());
            if fail_install.exists() {
                return Err("The password was not given, so nothing was installed.".into());
            }
            *fix_machine.lock().unwrap() = SystemProbe {
                postgres_installed: true,
                postgres_running: true,
                postgres_user: true,
                python: Some((3, 12)),
                python_venv: true,
                git: true,
                chrome: true,
                pdf_tools: true,
            };
            Ok("== Done".into())
        }),
        home: home.clone(),
        github: Arc::new(|_| GithubStatus {
            installed: true,
            connected: false,
            enterprise: false,
        }),
        user: "ana".into(),
    };
    let state = AppState::new(Workspace::new(&root), claude, settings)
        .with_odoo(odoo)
        .with_setup(hooks)
        .with_probe(test_probe)
        .with_providers(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            answers.lock().unwrap().clone()
        })
        .with_launcher(move |cmd| {
            record.lock().unwrap().push(cmd.to_vec());
            Ok(())
        });
    let app = register(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    Harness {
        launched,
        machine,
        scripts,
        assistants: statuses,
        assistant_checks,
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

    assert_eq!(h.call("running_sessions", json!({})).unwrap(), json!([]));
    let events = h.events_until_finished(|| {
        send().unwrap();
        let err = send().unwrap_err();
        assert!(err.contains("still answering"), "{err}");
        // The UI asks which sessions are busy when it opens one (e.g. after a reload).
        assert_eq!(h.call("running_sessions", json!({})).unwrap(), json!([id]));
        h.call("cancel_turn", json!({"sessionId": id})).unwrap();
    });
    assert_eq!(
        events.last().unwrap()["event"],
        json!({"kind": "finished", "is_error": true, "message": "Stopped."})
    );
    assert_eq!(h.call("running_sessions", json!({})).unwrap(), json!([]));
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
    assert!(err.contains("Claude is not installed"), "{err}");
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

#[test]
fn settings_default_then_round_trip() {
    let h = harness(true);
    let defaults = h.call("get_settings", json!({})).unwrap();
    assert_eq!(defaults["provider"], "claude");
    assert_eq!(defaults["defaultVersion"], "19.0");
    assert_eq!(defaults["demoData"], true);

    let new = json!({"provider": "claude", "defaultVersion": "18.0", "demoData": false});
    h.call("save_settings", json!({ "settings": new })).unwrap();
    assert_eq!(h.call("get_settings", json!({})).unwrap(), new);
}

#[test]
fn check_environment_reports_each_requirement() {
    let h = harness(true);
    let report = h.call("check_environment", json!({})).unwrap();
    let checks = report["checks"].as_array().unwrap();
    let keys: Vec<&str> = checks.iter().map(|c| c["key"].as_str().unwrap()).collect();
    assert_eq!(
        keys,
        ["python", "git", "postgresql", "odoo_source", "chrome", "ai"]
    );
    assert!(checks.iter().all(|c| c["status"] == "ok"), "{report}");
}

#[test]
fn list_models_offers_every_provider_and_locks_those_not_ready() {
    let h = harness(true);
    let models = h.call("list_models", json!({})).unwrap();
    let models = models.as_array().unwrap();
    let first = &models[0];
    assert_eq!(
        (first["id"].as_str(), first["locked"].as_bool()),
        (Some("sonnet"), Some(false))
    );
    let agy: Vec<_> = models
        .iter()
        .filter(|m| m["provider"] == "Antigravity")
        .collect();
    assert_eq!(agy.len(), 6);
    assert!(agy.iter().all(|m| m["locked"] == true));
}

#[test]
fn antigravity_models_unlock_once_agy_is_ready() {
    let h = harness_with(true, Some(fake_claude()));
    let models = h.call("list_models", json!({})).unwrap();
    assert!(models
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["locked"] == false));
}

#[test]
fn choosing_a_locked_model_explains_how_to_unlock_it() {
    let h = harness(true);
    let id = h.new_session();
    let err = h
        .call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "hi", "model": "claude-sonnet-4-6"}),
        )
        .unwrap_err();
    assert!(err.contains("Antigravity"), "{err}");
}

#[test]
fn provider_status_reports_each_assistant() {
    let h = harness(true);
    let status = h.call("provider_status", json!({})).unwrap();
    assert_eq!(
        status,
        json!([
            {"provider": "claude", "name": "Claude", "installed": true, "loggedIn": true},
            {"provider": "antigravity", "name": "Antigravity", "installed": true, "loggedIn": false},
        ])
    );
}

#[test]
fn logging_in_opens_the_providers_login_command() {
    let h = harness(true);
    h.call("login_provider", json!({"provider": "antigravity"}))
        .unwrap();
    h.call("login_provider", json!({"provider": "claude"}))
        .unwrap();
    let launched = h.launched.lock().unwrap().clone();
    assert_eq!(launched[0], ["/bin/agy"]);
    assert_eq!(launched[1][1..], ["auth", "login"]);
}

#[test]
fn logging_in_needs_the_cli_installed() {
    let h = harness(false);
    let err = h
        .call("login_provider", json!({"provider": "claude"}))
        .unwrap_err();
    assert!(err.contains("not installed"), "{err}");
}

#[test]
fn a_message_can_carry_an_effort_level() {
    let h = harness(true);
    let id = h.new_session();
    let events = h.events_until_finished(|| {
        h.call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "Spec it", "model": "opus", "effort": "high"}),
        )
        .unwrap();
    });
    assert!(events.last().unwrap()["event"]["kind"] == "finished");
}

#[test]
fn with_only_antigravity_installed_a_gemini_message_works() {
    let h = harness_with(false, Some(fake_agy()));
    let id = h.new_session();
    let events = h.events_until_finished(|| {
        h.call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "x", "model": "gemini-3.8-flash-low"}),
        )
        .unwrap();
    });
    assert_eq!(
        events.last().unwrap()["event"],
        json!({"kind": "finished", "is_error": false, "message": null})
    );
    let data = h
        .call("load_session", json!({"project": "Acme", "sessionId": id}))
        .unwrap();
    assert_eq!(
        data["meta"]["provider"], "antigravity",
        "the session is now Antigravity's"
    );
    assert_eq!(data["meta"]["started"], true);
}

#[test]
fn a_started_session_keeps_its_provider() {
    let h = harness_with(true, Some(fake_agy()));
    let id = h.new_session();
    h.events_until_finished(|| {
        h.call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "x", "model": "sonnet"}),
        )
        .unwrap();
    });
    let err = h
        .call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "y", "model": "gemini-3.8-flash-low"}),
        )
        .unwrap_err();
    assert!(err.contains("This session uses Claude"), "{err}");
}

#[test]
fn databases_are_listed_and_a_session_keeps_its_choice() {
    let h = harness(true);
    let list = h.call("list_databases", json!({})).unwrap();
    assert_eq!(
        list.as_array().unwrap().len(),
        1,
        "only the app's own: {list}"
    );
    assert_eq!(list[0]["name"], "specs_19_0_sale_demo");
    assert_eq!(list[0]["version"], "19.0");
    assert_eq!(list[0]["sizeBytes"], 5242880);
    assert_eq!(list[0]["auto"], true);

    let id = h.new_session();
    h.call(
        "set_session_database",
        json!({"project": "Acme", "sessionId": id, "database": "specs_19_0_sale_demo"}),
    )
    .unwrap();
    let projects = h.call("list_projects", json!({})).unwrap();
    assert_eq!(
        projects[0]["sessions"][0]["database"],
        "specs_19_0_sale_demo"
    );
    h.call(
        "set_session_database",
        json!({"project": "Acme", "sessionId": id, "database": null}),
    )
    .unwrap();
    let projects = h.call("list_projects", json!({})).unwrap();
    assert_eq!(projects[0]["sessions"][0]["database"], Value::Null);
}

#[test]
fn the_new_database_form_suggests_the_next_name_and_the_editions() {
    let h = harness(true);
    let form = h.call("database_form", json!({"version": "19.0"})).unwrap();
    assert_eq!(form["suggestedLabel"], "db-19-1");
    assert_eq!(form["enterpriseAvailable"], false);
    assert!(h.call("database_form", json!({"version": "nope"})).is_err());
}

#[test]
fn database_mistakes_come_back_as_readable_messages() {
    let h = harness(true);
    let err = h
        .call("delete_database", json!({"name": "other_db"}))
        .unwrap_err();
    assert!(err.contains("Only databases made by this app"), "{err}");
    let err = h
        .call(
            "create_database",
            json!({"database": {"label": " ", "version": "19.0", "modules": [], "demoData": true}}),
        )
        .unwrap_err();
    assert!(err.contains("Give the database a name"), "{err}");
}

#[test]
fn a_fresh_computer_is_set_up_with_one_password_prompt() {
    let h = harness(true);
    let status = h.call("setup_status", json!({})).unwrap();
    assert_eq!(status["system"]["ready"], false);
    assert_eq!(status["versions"], json!([]), "no Odoo here yet");
    assert_eq!(status["aiReady"], true);

    let events = collect_events(&h, "setup-progress", || {
        h.call("install_tools", json!({})).unwrap();
    });
    let scripts = h.scripts.lock().unwrap().clone();
    assert_eq!(scripts.len(), 1, "one password prompt");
    assert!(
        scripts[0].contains("apt-get install -y postgresql"),
        "{}",
        scripts[0]
    );
    assert!(scripts[0].contains("createuser --superuser 'ana'"));
    assert_eq!(events.last().unwrap()["task"]["state"], "done");
    let status = h.call("setup_status", json!({})).unwrap();
    assert_eq!(status["system"]["ready"], true);
    assert_eq!(status["tasks"]["tools"]["state"], "done");

    // Nothing missing any more: no password asked again.
    h.call("install_tools", json!({})).unwrap();
    assert_eq!(h.scripts.lock().unwrap().len(), 1);
}

#[test]
fn a_cancelled_password_is_reported_and_can_be_retried() {
    let h = harness(true);
    fs::write(h._dir.path().join("fail-install"), "").unwrap();
    let err = h.call("install_tools", json!({})).unwrap_err();
    assert!(err.contains("password was not given"), "{err}");
    let status = h.call("setup_status", json!({})).unwrap();
    assert_eq!(status["tasks"]["tools"]["state"], "failed");
    fs::remove_file(h._dir.path().join("fail-install")).unwrap();
    h.call("install_tools", json!({})).unwrap();
    assert!(h.machine.lock().unwrap().postgres_user);
}

fn wait_for_task(h: &Harness, key: &str) -> Value {
    for _ in 0..100 {
        let status = h.call("setup_status", json!({})).unwrap();
        let task = status["tasks"][key].clone();
        if task["state"] != "running" && !task.is_null() {
            return task;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{key} never finished");
}

#[test]
fn enterprise_downloads_in_the_background_or_says_there_is_no_access_yet() {
    let h = harness(true);
    fs::write(h._dir.path().join("deny"), "").unwrap();
    h.call(
        "download_odoo",
        json!({"version": "19.0", "edition": "enterprise"}),
    )
    .unwrap();
    let task = wait_for_task(&h, "enterprise:19.0");
    assert_eq!(task["state"], "noAccess");
    assert!(task["message"]
        .as_str()
        .unwrap()
        .contains("no access to Odoo Enterprise yet"));

    // Access granted later: Try again.
    fs::remove_file(h._dir.path().join("deny")).unwrap();
    h.call(
        "download_odoo",
        json!({"version": "19.0", "edition": "enterprise"}),
    )
    .unwrap();
    assert_eq!(wait_for_task(&h, "enterprise:19.0")["state"], "done");
    let versions = h.call("odoo_versions", json!({})).unwrap();
    assert_eq!(versions["downloaded"][0]["enterprise"]["state"], "ready");
}

#[test]
fn connecting_github_makes_the_apps_key_and_adds_it_with_one_sign_in() {
    let h = harness(true);
    h.call("github_connect", json!({})).unwrap();
    let key = h._dir.path().join("runtime/ssh/id_ed25519.pub");
    assert!(key.is_file(), "the app's own key, made once");
    let launched = h.launched.lock().unwrap().clone().concat().join(" ");
    assert!(launched.contains("gh auth"), "{launched}");
    assert!(
        launched.contains("admin:public_key"),
        "allowed to add the key: {launched}"
    );
    assert!(launched.contains("gh ssh-key add"), "{launched}");
    assert!(launched.contains(&key.display().to_string()), "{launched}");

    let err = h
        .call(
            "use_odoo_folder",
            json!({"version": "19.0", "edition": "community", "path": "/tmp"}),
        )
        .unwrap_err();
    assert!(err.contains("is not an Odoo source folder"), "{err}");
}

#[test]
fn the_versions_list_shows_what_odoo_publishes_and_what_is_here() {
    let h = harness(true);
    h.call(
        "download_odoo",
        json!({"version": "saas-19.2", "edition": "enterprise"}),
    )
    .unwrap();
    for _ in 0..100 {
        let s = h.call("setup_status", json!({})).unwrap();
        if s["tasks"]["enterprise:saas-19.2"]["state"] == "done" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // What Odoo publishes comes separately: it needs GitHub and may take a while.
    let published = h.call("odoo_published_versions", json!({})).unwrap();
    assert_eq!(
        published["available"],
        json!(["saas-19.2", "saas-19.1", "19.0", "18.0"])
    );
    assert_eq!(published["latestSaas"], "saas-19.2");
    // What is here answers at once, with where its files are and where downloads go.
    let versions = h.call("odoo_versions", json!({})).unwrap();
    assert_eq!(versions["downloaded"][0]["version"], "saas-19.2");
    let folder = versions["downloadsFolder"].as_str().unwrap();
    assert!(folder.ends_with("odoo-versions"), "{folder}");
    assert_eq!(
        versions["downloaded"][0]["enterprisePath"],
        format!("{folder}/saas-19.2/enterprise")
    );
    h.call("remove_odoo_version", json!({"version": "saas-19.2"}))
        .unwrap();
    let versions = h.call("odoo_versions", json!({})).unwrap();
    assert_eq!(versions["downloaded"], json!([]));
}

#[test]
fn a_session_keeps_its_hosting_and_new_ones_follow_the_client() {
    let h = harness(true);
    let id = h.new_session();
    h.call(
        "set_session_hosting",
        json!({"project": "Acme", "sessionId": id, "hosting": "online"}),
    )
    .unwrap();
    let projects = h.call("list_projects", json!({})).unwrap();
    assert_eq!(projects[0]["sessions"][0]["hosting"], "online");
    let next = h
        .call(
            "create_session",
            json!({"project": "Acme", "title": "Next"}),
        )
        .unwrap();
    assert_eq!(next["hosting"], "online");
    let err = h
        .call(
            "set_session_hosting",
            json!({"project": "Acme", "sessionId": id, "hosting": "cloud"}),
        )
        .unwrap_err();
    assert!(err.contains("Unknown hosting"), "{err}");
}

#[test]
fn a_session_keeps_its_odoo_version() {
    let h = harness(true);
    let id = h.new_session();
    h.call(
        "set_session_version",
        json!({"project": "Acme", "sessionId": id, "version": "saas-19.2"}),
    )
    .unwrap();
    let projects = h.call("list_projects", json!({})).unwrap();
    assert_eq!(projects[0]["sessions"][0]["version"], "saas-19.2");
    assert!(h
        .call(
            "set_session_version",
            json!({"project": "Acme", "sessionId": id, "version": "latest"}),
        )
        .unwrap_err()
        .contains("is not an Odoo version"));
}

/// A fake Odoo 19.0 checkout (with Enterprise) where odev keeps it.
fn odev_worktree(h: &Harness, version: &str) {
    let dir = h._dir.path().join("home/odev/worktrees").join(version);
    fs::create_dir_all(dir.join("odoo/odoo")).unwrap();
    fs::write(dir.join("odoo/odoo-bin"), "").unwrap();
    let (major, minor) = version.split_once('.').unwrap();
    fs::write(
        dir.join("odoo/odoo/release.py"),
        format!("version_info = ({major}, {minor}, 0, 'final', 0, '')\n"),
    )
    .unwrap();
    fs::create_dir_all(dir.join("enterprise/web_enterprise")).unwrap();
}

#[test]
fn the_setup_finds_odoo_on_the_computer_and_uses_it_without_downloading() {
    let h = harness(true);
    odev_worktree(&h, "19.0");
    odev_worktree(&h, "18.0");
    let status = h.call("setup_status", json!({})).unwrap();
    assert_eq!(status["versions"], json!([]));
    let found: Vec<_> = status["found"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["version"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(found, ["19.0", "18.0"]);

    let used = h.call("use_found_sources", json!({})).unwrap();
    assert_eq!(used, json!(["19.0", "18.0"]));
    let status = h.call("setup_status", json!({})).unwrap();
    assert_eq!(status["versions"], json!(["19.0", "18.0"]));
    assert_eq!(status["found"], json!([]), "nothing left to use");
}

// --- Sending is quick: the assistants are not checked again for every message ---

#[test]
fn messages_reuse_a_recent_check_of_the_assistants() {
    let h = harness(true);
    let id = h.new_session();
    let send = |text: &str| {
        h.events_until_finished(|| {
            h.call(
                "send_message",
                json!({"project": "Acme", "sessionId": id, "text": text}),
            )
            .unwrap();
        });
    };
    send("first");
    let after_first = h.assistant_checks.load(std::sync::atomic::Ordering::SeqCst);
    send("second");
    send("third");
    assert_eq!(
        h.assistant_checks.load(std::sync::atomic::Ordering::SeqCst),
        after_first,
        "checking means running both CLIs (over a second with agy)"
    );
}

#[test]
fn logging_in_meanwhile_is_seen_at_once() {
    let h = harness(false);
    let id = h.new_session();
    let refused = h
        .call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "hi"}),
        )
        .unwrap_err();
    assert!(refused.contains("not"), "{refused}");
    // The person logs in, in the terminal the app opened.
    h.assistants.lock().unwrap()[0] = status(Provider::Claude, Some(fake_claude()), true);
    let events = h.events_until_finished(|| {
        h.call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "hi"}),
        )
        .unwrap();
    });
    assert_eq!(events.last().unwrap()["event"]["kind"], "finished");
}

// --- Attachments ---

#[test]
fn an_attached_report_is_mentioned_to_the_assistant_and_only_pdfs_are_taken() {
    let h = harness(true);
    let id = h.new_session();
    let doc = h.root.join("notes.docx");
    fs::write(&doc, "x").unwrap();
    let err = h
        .call(
            "attach_file",
            json!({"project": "Acme", "sessionId": id, "path": doc}),
        )
        .unwrap_err();
    assert!(err.contains("Only PDF"), "{err}");

    let attachment = json!({"name": "quotation.pdf", "textFile": "attachments/quotation.md",
        "layoutImage": "attachments/quotation-layout.png", "pages": 2, "removed": 3, "scanned": false});
    h.events_until_finished(|| {
        h.call(
            "send_message",
            json!({"project": "Acme", "sessionId": id, "text": "Map this report",
                   "attachments": [attachment]}),
        )
        .unwrap();
    });
    let data = h
        .call("load_session", json!({"project": "Acme", "sessionId": id}))
        .unwrap();
    let sent = data["transcript"][0]["text"].as_str().unwrap();
    assert!(sent.starts_with("Map this report"), "{sent}");
    assert!(sent.contains("attachments/quotation.md"), "{sent}");
}

#[test]
fn the_setup_lists_the_versions_in_use_with_their_folders() {
    let h = harness(true);
    let mine = h.root.join("odev/worktrees/19.0");
    fs::create_dir_all(mine.join("odoo/odoo")).unwrap();
    fs::write(mine.join("odoo/odoo-bin"), "").unwrap();
    fs::write(
        mine.join("odoo/odoo/release.py"),
        "version_info = (19, 0, 0, 'final', 0, '')\n",
    )
    .unwrap();
    fs::create_dir_all(mine.join("enterprise/web_enterprise")).unwrap();
    let version = h.call("use_source_folder", json!({"path": mine})).unwrap();
    assert_eq!(version, "19.0");
    let status = h.call("setup_status", json!({})).unwrap();
    assert_eq!(status["versions"], json!(["19.0"]));
    let source = &status["sources"][0];
    assert_eq!(
        source["communityPath"],
        mine.join("odoo").display().to_string()
    );
    assert_eq!(
        source["enterprisePath"],
        mine.join("enterprise").display().to_string()
    );
}
