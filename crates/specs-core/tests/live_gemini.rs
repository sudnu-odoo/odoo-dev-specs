//! End-to-end against the real, API-key-authenticated Gemini CLI. Costs tokens,
//! so it is skipped by default. Run with: `make test-live`

use specs_core::chat::{send_message, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::locate::{find_executable, user_bin_dirs};
use specs_core::settings::Provider;
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::Workspace;
use tokio::sync::oneshot;

fn config() -> ClaudeConfig {
    let home = dirs::home_dir().unwrap();
    let path = std::env::var("PATH").ok();
    let bins = user_bin_dirs(&home);
    let claude =
        find_executable("claude", path.as_deref(), &bins).expect("Claude Code is not installed");
    let gemini =
        find_executable("gemini", path.as_deref(), &bins).expect("Gemini CLI is not installed");
    ClaudeConfig {
        program: claude,
        gemini_program: Some(gemini),
        provider: Provider::Gemini,
        read_dirs: vec![],
        model: Some("gemini-2.5-flash".into()),
        diagram_tools: None,
        odev: None,
        demo_data: true,
    }
}

#[tokio::test]
#[ignore = "calls the real Gemini CLI (API key); run with `make test-live`"]
async fn real_gemini_answers_and_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "gemini").unwrap().id;
    let cfg = config();

    let (_tx, rx) = oneshot::channel();
    let first = send_message(
        &ws,
        "P",
        &id,
        "Remember the word 'tangerine'. Reply with exactly: ok",
        &cfg,
        |_| {},
        rx,
    )
    .await
    .unwrap();
    assert_eq!(
        first,
        ChatEvent::Finished {
            is_error: false,
            message: None
        },
        "transcript: {:#?}",
        ws.read_transcript("P", &id).unwrap()
    );
    assert!(ws.session("P", &id).unwrap().started);

    let (_tx2, rx2) = oneshot::channel();
    let second = send_message(
        &ws,
        "P",
        &id,
        "Which word did I ask you to remember? One word.",
        &cfg,
        |_| {},
        rx2,
    )
    .await
    .unwrap();
    assert_eq!(
        second,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );
    let last = ws.read_transcript("P", &id).unwrap().pop();
    match last {
        Some(TranscriptEntry::Assistant { text }) => {
            assert!(
                text.to_lowercase().contains("tangerine"),
                "resume lost context: {text}"
            )
        }
        other => panic!("expected an answer, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "calls the real Gemini CLI + diagram tools; run with `make test-live`"]
async fn real_gemini_draws_with_the_diagram_tools() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    assert!(mcp.is_file(), "build it first: cargo build -p specs-mcp");
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "gemini").unwrap().id;
    let cfg = ClaudeConfig {
        diagram_tools: Some(mcp),
        ..config()
    };

    let (_tx, rx) = oneshot::channel();
    let done = send_message(
        &ws,
        "P",
        &id,
        "Use the diagram tools: add a blue box labelled 'Sales order' and a green box labelled \
         'Delivery', then an arrow from the first to the second. Then reply: done",
        &cfg,
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
    assert!(count("rectangle") >= 2, "boxes drawn: {scene}");
    let tools: Vec<String> = ws
        .read_transcript("P", &id)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            TranscriptEntry::Tool { name, .. } => Some(name),
            _ => None,
        })
        .collect();
    assert!(
        tools.iter().any(|t| t.contains("diagram_add_box")),
        "{tools:?}"
    );
}
