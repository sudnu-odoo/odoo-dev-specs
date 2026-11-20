//! End-to-end against the real, logged-in Antigravity CLI (`agy`). Uses real quota,
//! so it is skipped by default. Run with: `make test-live`

use specs_core::chat::{send_message, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::locate::{find_executable, user_bin_dirs};
use specs_core::settings::Provider;
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::Workspace;
use tokio::sync::oneshot;

fn real_agy() -> ClaudeConfig {
    let home = dirs::home_dir().unwrap();
    let path = std::env::var("PATH").ok();
    let program = find_executable("agy", path.as_deref(), &user_bin_dirs(&home))
        .expect("Antigravity (agy) is not installed");
    ClaudeConfig {
        program,
        provider: Provider::Antigravity,
        read_dirs: vec![],
        model: Some("gemini-3.8-flash-low".into()),
        effort: None,
        diagram_tools: None,
        runtime_dir: None,
        demo_data: true,
    }
}

#[tokio::test]
#[ignore = "calls the real Antigravity CLI; run with `make test-live`"]
async fn real_agy_answers_and_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "antigravity").unwrap().id;
    let agy = real_agy();
    let ok = ChatEvent::Finished {
        is_error: false,
        message: None,
    };
    let turn = |text: &'static str| {
        let (ws, id, agy) = (&ws, &id, &agy);
        async move {
            let (_tx, rx) = oneshot::channel();
            send_message(ws, "P", id, text, agy, |_| {}, rx)
                .await
                .unwrap()
        }
    };

    assert_eq!(
        turn("Remember the word 'tangerine'. Reply with exactly: ok").await,
        ok
    );
    assert!(ws.session("P", &id).unwrap().conversation_id.is_some());

    assert_eq!(
        turn("Which word did I ask you to remember? One word.").await,
        ok
    );
    match ws.read_transcript("P", &id).unwrap().pop() {
        Some(TranscriptEntry::Assistant { text }) => {
            assert!(text.to_lowercase().contains("tangerine"), "{text}");
        }
        other => panic!("expected an assistant reply, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "calls the real Antigravity CLI; run with `make test-live`"]
async fn real_agy_draws_with_the_diagram_tools() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    assert!(mcp.is_file(), "build it first: cargo build -p specs-mcp");
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "antigravity").unwrap().id;
    let agy = ClaudeConfig {
        diagram_tools: Some(mcp),
        model: Some("gemini-3.8-flash-medium".into()),
        ..real_agy()
    };

    let (_tx, rx) = oneshot::channel();
    let done = send_message(
        &ws,
        "P",
        &id,
        "Using the diagram tools, add a blue box 'Sales order', a green box 'Delivery', \
         and an arrow from the first to the second labelled 'plans'. Then reply: done",
        &agy,
        |_| {},
        rx,
    )
    .await
    .unwrap();
    assert_eq!(
        done,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );

    let scene: serde_json::Value = serde_json::from_str(&ws.read_scene("P", &id).unwrap()).unwrap();
    let live: Vec<&serde_json::Value> = scene["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isDeleted"] != true)
        .collect();
    let count = |kind: &str| live.iter().filter(|e| e["type"] == kind).count();
    assert_eq!((count("rectangle"), count("arrow")), (2, 1), "{scene}");
    let texts: Vec<&str> = live.iter().filter_map(|e| e["text"].as_str()).collect();
    for expected in ["Sales order", "Delivery", "plans"] {
        assert!(texts.contains(&expected), "missing {expected}: {texts:?}");
    }
}

async fn turn(ws: &Workspace, id: &str, agy: &ClaudeConfig, text: &str) -> ChatEvent {
    let (_tx, rx) = oneshot::channel();
    send_message(ws, "P", id, text, agy, |_| {}, rx)
        .await
        .unwrap()
}

/// The same request as the Claude live test, to compare both assistants.
#[tokio::test]
#[ignore = "calls the real Antigravity CLI; run with `make test-live`"]
async fn real_agy_starts_a_screen_change_from_a_screenshot() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    let runtime = odoo_runtime::runtime::Runtime::new(
        odoo_runtime::runtime::Runtime::default_root(),
        Default::default(),
    );
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "antigravity").unwrap().id;
    ws.set_hosting("P", &id, Some("online")).unwrap();
    ws.set_version("P", &id, Some("19.0")).unwrap();
    // The model the business team gets by default.
    let agy = ClaudeConfig {
        diagram_tools: Some(mcp),
        runtime_dir: Some(odoo_runtime::runtime::Runtime::default_root()),
        // The Odoo source of the session's version, as the app passes it.
        read_dirs: runtime.source_dirs_for("19.0"),
        // AGY_MODEL picks another model, e.g. when the Gemini quota is used up.
        model: std::env::var("AGY_MODEL").ok(),
        ..real_agy()
    };

    let started = std::time::Instant::now();
    let done = turn(
        &ws,
        &id,
        &agy,
        "Add a new button beside the Confirm button of sale orders called \"Revise Quotation\"",
    )
    .await;
    // Answer its questions like the business analyst would.
    let done =
        match done {
            ChatEvent::Finished {
                is_error: false, ..
            } if !used_spec_build(&ws, &id) => turn(
                &ws,
                &id,
                &agy,
                "1a, cancel the original. 2: only quotations not confirmed. 3: all salespeople. \
4: a Revisions smart button, no PDF change. 5: customers ask for changes after the quote. \
6: no past task. Please draw the spec.",
            )
            .await,
            other => other,
        };
    let took = started.elapsed();
    runtime.stop_all().unwrap();
    assert_eq!(
        done,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );
    let tools = tools(&ws, &id);
    eprintln!("took {took:?}; tools: {tools:?}");
    assert!(used_spec_build(&ws, &id), "{tools:?}");
    let keep =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-fixtures");
    std::fs::create_dir_all(&keep).unwrap();
    std::fs::write(
        keep.join("live_agy_revise_quotation.excalidraw"),
        ws.read_scene("P", &id).unwrap(),
    )
    .unwrap();
}

fn tools(ws: &Workspace, id: &str) -> Vec<String> {
    ws.read_transcript("P", id)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            TranscriptEntry::Tool { name, is_error } => {
                Some(format!("{name}{}", if is_error { " (error)" } else { "" }))
            }
            _ => None,
        })
        .collect()
}

fn used_spec_build(ws: &Workspace, id: &str) -> bool {
    tools(ws, id).iter().any(|t| t.ends_with("spec_build"))
}
