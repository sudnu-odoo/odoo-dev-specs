//! End-to-end against the real, logged-in Claude Code. Costs real tokens, so it is
//! skipped by default. Run with: `make test-live`

use specs_core::chat::{send_message, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::locate::{find_executable, user_bin_dirs};
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::Workspace;
use tokio::sync::oneshot;

fn real_claude() -> ClaudeConfig {
    let home = dirs::home_dir().unwrap();
    let path = std::env::var("PATH").ok();
    let program = find_executable("claude", path.as_deref(), &user_bin_dirs(&home))
        .expect("Claude Code is not installed");
    ClaudeConfig {
        program,
        read_dirs: vec![],
        model: Some("haiku".into()),
        diagram_tools: None,
    }
}

async fn turn(ws: &Workspace, id: &str, claude: &ClaudeConfig, text: &str) -> ChatEvent {
    let (_tx, rx) = oneshot::channel();
    send_message(ws, "P", id, text, claude, |_| {}, rx)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "calls the real Claude Code; run with `make test-live`"]
async fn real_claude_answers_and_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    let claude = real_claude();

    let first = turn(
        &ws,
        &id,
        &claude,
        "Remember the word 'tangerine'. Reply with exactly: ok",
    )
    .await;
    assert_eq!(
        first,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );
    assert!(ws.session("P", &id).unwrap().started);

    let second = turn(
        &ws,
        &id,
        &claude,
        "Which word did I ask you to remember? One word.",
    )
    .await;
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
#[ignore = "calls the real Claude Code; run with `make test-live`"]
async fn real_claude_draws_with_the_diagram_tools() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    assert!(mcp.is_file(), "build it first: cargo build -p specs-mcp");
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    let claude = ClaudeConfig {
        diagram_tools: Some(mcp),
        ..real_claude()
    };

    let done = turn(
        &ws,
        &id,
        &claude,
        "Using the diagram tools, add a blue box 'Sales order', a green box 'Delivery', \
         and an arrow from the first to the second labelled 'plans'. Then reply: done",
    )
    .await;
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
    let tools: Vec<_> = ws
        .read_transcript("P", &id)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            TranscriptEntry::Tool { name, .. } => Some(name),
            _ => None,
        })
        .collect();
    assert!(
        tools.iter().any(|t| t == "mcp__specs__diagram_add_arrow"),
        "{tools:?}"
    );
}
