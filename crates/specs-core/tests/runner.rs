//! Runner against fake CLI scripts (no real AI involved).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use specs_core::claude::parse_line;
use specs_core::events::ChatEvent;
use specs_core::runner::{run_turn, CliTurn};
use tempfile::TempDir;
use tokio::sync::oneshot;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/claude_read_tool.jsonl"
);

fn fake_cli(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("fake-claude");
    fs::write(&path, format!("{body}\n")).unwrap();
    path
}

/// The fake script runs through `/bin/sh` rather than being executed directly:
/// executing a file that was just written races with other test threads forking
/// (the inherited write handle makes exec fail with "Text file busy").
fn turn(dir: &TempDir, script: PathBuf) -> CliTurn {
    let args = ["-p", "--flag", "value with spaces"].map(String::from);
    CliTurn {
        program: "/bin/sh".into(),
        args: std::iter::once(script.display().to_string())
            .chain(args)
            .collect(),
        cwd: dir.path().to_path_buf(),
        prompt: "Spec the delivery window\nplease".into(),
    }
}

async fn collect(turn: CliTurn, cancel: oneshot::Receiver<()>) -> Vec<ChatEvent> {
    let mut events = Vec::new();
    run_turn(turn, parse_line, |e| events.push(e), cancel).await;
    events
}

fn finished(events: &[ChatEvent]) -> Vec<&ChatEvent> {
    events
        .iter()
        .filter(|e| matches!(e, ChatEvent::Finished { .. }))
        .collect()
}

fn error_message(events: &[ChatEvent]) -> String {
    match events.last() {
        Some(ChatEvent::Finished {
            is_error: true,
            message: Some(m),
        }) => m.clone(),
        other => panic!("expected a failed Finished, got {other:?}"),
    }
}

#[tokio::test]
async fn streams_events_and_passes_prompt_args_and_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let cli = fake_cli(
        dir.path(),
        &format!(
            r#"cat > stdin.txt; printf '%s\n' "$@" > args.txt; pwd > cwd.txt; cat '{FIXTURE}'"#
        ),
    );
    let (_tx, rx) = oneshot::channel();
    let events = collect(turn(&dir, cli), rx).await;

    let expected: Vec<ChatEvent> = fs::read_to_string(FIXTURE)
        .unwrap()
        .lines()
        .flat_map(parse_line)
        .collect();
    assert_eq!(events, expected);
    let read = |f: &str| fs::read_to_string(dir.path().join(f)).unwrap();
    assert_eq!(read("stdin.txt"), "Spec the delivery window\nplease");
    assert_eq!(read("args.txt"), "-p\n--flag\nvalue with spaces\n");
    assert_eq!(
        read("cwd.txt").trim(),
        dir.path().canonicalize().unwrap().to_str().unwrap()
    );
}

#[tokio::test]
async fn crash_reports_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let cli = fake_cli(
        dir.path(),
        "cat >/dev/null; echo 'Not logged in. Please run /login' >&2; exit 1",
    );
    let (_tx, rx) = oneshot::channel();
    let events = collect(turn(&dir, cli), rx).await;
    assert_eq!(finished(&events).len(), 1);
    assert!(error_message(&events).contains("Not logged in"));
}

#[tokio::test]
async fn clean_exit_without_result_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let cli = fake_cli(dir.path(), "cat >/dev/null; exit 0");
    let (_tx, rx) = oneshot::channel();
    let events = collect(turn(&dir, cli), rx).await;
    assert_eq!(finished(&events).len(), 1);
    assert!(error_message(&events).contains("ended unexpectedly"));
}

#[tokio::test]
async fn result_followed_by_nonzero_exit_finishes_once() {
    let dir = tempfile::tempdir().unwrap();
    let cli = fake_cli(
        dir.path(),
        &format!("cat >/dev/null; cat '{FIXTURE}'; exit 3"),
    );
    let (_tx, rx) = oneshot::channel();
    let events = collect(turn(&dir, cli), rx).await;
    assert_eq!(
        finished(&events),
        vec![&ChatEvent::Finished {
            is_error: false,
            message: None
        }]
    );
}

#[tokio::test]
async fn missing_program_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (_tx, rx) = oneshot::channel();
    let missing = CliTurn {
        program: dir.path().join("no-such-cli"),
        ..turn(&dir, PathBuf::new())
    };
    let events = collect(missing, rx).await;
    assert_eq!(events.len(), 1);
    assert!(error_message(&events).contains("could not be started"));
}

#[tokio::test]
async fn cancel_kills_the_process() {
    let dir = tempfile::tempdir().unwrap();
    // `sleep` runs as a grandchild; it must die with the CLI, not linger.
    let cli = fake_cli(dir.path(), "cat >/dev/null; sleep 30.4242; echo after");
    let (tx, rx) = oneshot::channel();
    let start = Instant::now();
    let run = tokio::spawn(collect(turn(&dir, cli), rx));
    tokio::time::sleep(Duration::from_millis(200)).await;
    tx.send(()).unwrap();
    let events = run.await.unwrap();
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "took {:?}",
        start.elapsed()
    );
    assert_eq!(
        events,
        vec![ChatEvent::Finished {
            is_error: true,
            message: Some("Stopped.".into())
        }]
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    let lingering = std::process::Command::new("pgrep")
        .args(["-f", "sleep 30.4242"])
        .output()
        .unwrap();
    assert!(lingering.stdout.is_empty(), "child process survived cancel");
}
