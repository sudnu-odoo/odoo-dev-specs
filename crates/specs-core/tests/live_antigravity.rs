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
