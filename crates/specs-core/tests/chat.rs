//! Full chat turns against a temporary workspace and a fake `claude`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::OnceLock;

use specs_core::chat::{send_message, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::{SessionMeta, Workspace};
use tempfile::TempDir;
use tokio::sync::oneshot;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/claude_read_tool.jsonl"
);

const AGY_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/agy_read_tool.jsonl"
);

/// A fake CLI replaying a recorded run, written once before any test starts a process
/// (executing a freshly written file races with concurrent forks: "Text file busy").
/// It runs in the session folder: it logs its args there, and fails if a `fail` file exists.
fn write_fake(name: &str, fixture: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    let script = format!(
        "#!/bin/sh\ncat > stdin.txt\nprintf '%s\\n' \"$@\" > args.txt\n\
         if [ -f fail ]; then echo 'Not logged in' >&2; exit 1; fi\ncat '{fixture}'\n"
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    (dir, path)
}

fn fake_claude() -> PathBuf {
    static FAKE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    FAKE.get_or_init(|| write_fake("claude", FIXTURE)).1.clone()
}

fn fake_agy() -> PathBuf {
    static FAKE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    FAKE.get_or_init(|| write_fake("agy", AGY_FIXTURE))
        .1
        .clone()
}

struct Env {
    ws: Workspace,
    session: SessionMeta,
    config: ClaudeConfig,
    _dir: TempDir,
}

fn env() -> Env {
    let program = fake_claude();
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let session = ws.create_session("P", "t", "claude").unwrap();
    let config = ClaudeConfig {
        program,
        provider: specs_core::settings::Provider::Claude,
        read_dirs: vec!["/src/odoo".into()],
        model: None,
        effort: None,
        diagram_tools: Some("/app/specs-mcp".into()),
        runtime_dir: None,
        demo_data: true,
    };
    Env {
        ws,
        session,
        config,
        _dir: dir,
    }
}

impl Env {
    /// Streamed events, then the returned final event appended last.
    async fn send(&self, text: &str) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        let (_tx, rx) = oneshot::channel();
        let last = send_message(
            &self.ws,
            "P",
            &self.session.id,
            text,
            &self.config,
            |e| events.push(e),
            rx,
        )
        .await
        .unwrap();
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, ChatEvent::Finished { .. })),
            "finished must only be returned, after everything is saved"
        );
        assert!(matches!(last, ChatEvent::Finished { .. }));
        events.push(last);
        events
    }

    fn session_file(&self, name: &str) -> String {
        fs::read_to_string(
            self.ws
                .session_dir("P", &self.session.id)
                .unwrap()
                .join(name),
        )
        .unwrap()
    }

    fn args(&self) -> Vec<String> {
        self.session_file("args.txt")
            .lines()
            .map(String::from)
            .collect()
    }
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .map(|i| args[i + 1].clone())
}

#[tokio::test]
async fn first_turn_streams_saves_transcript_and_starts_session() {
    let env = env();
    let events = env.send("Add a delivery window").await;

    assert_eq!(
        events.last(),
        Some(&ChatEvent::Finished {
            is_error: false,
            message: None
        })
    );
    assert_eq!(env.session_file("stdin.txt"), "Add a delivery window");
    let args = env.args();
    assert_eq!(
        value_after(&args, "--session-id"),
        Some(env.session.id.clone())
    );
    assert_eq!(value_after(&args, "--add-dir"), Some("/src/odoo".into()));

    assert_eq!(
        env.ws.read_transcript("P", &env.session.id).unwrap(),
        vec![
            TranscriptEntry::User {
                text: "Add a delivery window".into()
            },
            TranscriptEntry::Tool {
                name: "Read".into(),
                is_error: false
            },
            TranscriptEntry::Assistant {
                text: "done".into()
            },
        ]
    );
    assert!(env.ws.session("P", &env.session.id).unwrap().started);
}

#[tokio::test]
async fn second_turn_resumes_the_conversation() {
    let env = env();
    env.send("one").await;
    env.send("two").await;
    let args = env.args();
    assert_eq!(value_after(&args, "--resume"), Some(env.session.id.clone()));
    assert_eq!(value_after(&args, "--session-id"), None);
    assert_eq!(
        env.ws.read_transcript("P", &env.session.id).unwrap().len(),
        6
    );
}

#[tokio::test]
async fn failed_first_turn_keeps_message_and_error_but_not_started() {
    let env = env();
    let dir = env.ws.session_dir("P", &env.session.id).unwrap();
    fs::write(dir.join("fail"), "").unwrap();

    let events = env.send("hello").await;
    assert!(matches!(
        events.last(),
        Some(ChatEvent::Finished { is_error: true, .. })
    ));
    assert_eq!(
        env.ws.read_transcript("P", &env.session.id).unwrap(),
        vec![
            TranscriptEntry::User {
                text: "hello".into()
            },
            TranscriptEntry::Error {
                message: "Not logged in".into()
            },
        ]
    );
    // Claude never created the conversation, so the next turn must not try to resume it.
    assert!(!env.ws.session("P", &env.session.id).unwrap().started);
}

#[tokio::test]
async fn unknown_session_is_an_error() {
    let env = env();
    let (_tx, rx) = oneshot::channel();
    let missing = "6f1c2a7e-0000-4000-8000-000000000000";
    let result = send_message(&env.ws, "P", missing, "x", &env.config, |_| {}, rx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn diagram_tools_work_on_this_session_folder() {
    let env = env();
    env.send("draw it").await;
    let args = env.args();
    let config: serde_json::Value =
        serde_json::from_str(&value_after(&args, "--mcp-config").unwrap()).unwrap();
    let dir = env.ws.session_dir("P", &env.session.id).unwrap();
    assert_eq!(config["mcpServers"]["specs"]["command"], "/app/specs-mcp");
    assert_eq!(
        config["mcpServers"]["specs"]["args"],
        serde_json::json!(["--session-dir", dir.to_str().unwrap()])
    );
}

#[tokio::test]
async fn antigravity_turn_remembers_its_conversation_and_resumes_it() {
    let mut e = env();
    e.config.program = fake_agy();
    e.config.provider = specs_core::settings::Provider::Antigravity;

    let events = e.send("Add a field").await;
    assert!(events.contains(&ChatEvent::Conversation {
        id: "2b27c3b2-dade-4994-93db-adb0dfdb7252".into()
    }));
    let args = e.args();
    assert_eq!(args[0], "-p");
    assert!(!args.contains(&"--conversation".to_string()));
    // Our diagram tools are offered through the session's MCP config.
    assert!(e
        .session_file(".agents/mcp_config.json")
        .contains("/app/specs-mcp"));

    let meta = e.ws.session("P", &e.session.id).unwrap();
    assert_eq!(
        meta.conversation_id.as_deref(),
        Some("2b27c3b2-dade-4994-93db-adb0dfdb7252")
    );

    e.send("And another").await;
    let args = e.args();
    let at = args.iter().position(|a| a == "--conversation").unwrap();
    assert_eq!(args[at + 1], "2b27c3b2-dade-4994-93db-adb0dfdb7252");
    assert_eq!(args[1], "And another");
}

#[tokio::test]
async fn chosen_effort_reaches_the_cli() {
    let mut e = env();
    e.config.effort = Some("xhigh".into());
    e.send("Spec it").await;
    let args = e.args();
    let at = args.iter().position(|a| a == "--effort").unwrap();
    assert_eq!(args[at + 1], "xhigh");
}
