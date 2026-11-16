//! Runs one chat turn of an AI CLI as a child process and streams its events.

use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::events::ChatEvent;

/// How much of stderr is shown to the user when the CLI fails.
const STDERR_TAIL: usize = 600;

/// One invocation of a provider CLI.
#[derive(Debug, Clone)]
pub struct CliTurn {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Sent on stdin, then stdin is closed.
    pub prompt: String,
}

/// Run the turn, sending every parsed event to `on_event`.
///
/// Always ends with exactly one [`ChatEvent::Finished`], also when the CLI is
/// missing, crashes, exits without a result, or `cancel` fires (the process is killed).
pub async fn run_turn(
    turn: CliTurn,
    parse: fn(&str) -> Vec<ChatEvent>,
    mut on_event: impl FnMut(ChatEvent) + Send,
    mut cancel: oneshot::Receiver<()>,
) {
    let mut child = match spawn(&turn) {
        Ok(child) => child,
        Err(e) => {
            on_event(failure(format!(
                "{} could not be started: {e}",
                turn.program.display()
            )));
            return;
        }
    };
    let process_group = child.id();

    if let Some(mut stdin) = child.stdin.take() {
        let prompt = turn.prompt;
        // Dropping stdin after the write closes it, which tells the CLI the prompt is complete.
        tokio::spawn(async move { stdin.write_all(prompt.as_bytes()).await });
    }
    let stderr = child.stderr.take().map(|mut err| {
        tokio::spawn(async move {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s).await;
            s
        })
    });
    let Some(stdout) = child.stdout.take() else {
        on_event(failure("Could not read the assistant's output.".into()));
        return;
    };
    let mut lines = BufReader::new(stdout).lines();

    let mut finished = false;
    // A dropped sender means nobody can cancel any more; stop polling it.
    let mut can_cancel = true;
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    for event in parse(&line) {
                        if finished {
                            break;
                        }
                        finished = matches!(event, ChatEvent::Finished { .. });
                        on_event(event);
                    }
                }
                _ => break,
            },
            sent = &mut cancel, if can_cancel => {
                if sent.is_ok() {
                    kill_group(process_group);
                    let _ = child.wait().await;
                    if !finished {
                        on_event(failure("Stopped.".into()));
                    }
                    return;
                }
                can_cancel = false;
            }
        }
    }

    let status = child.wait().await;
    // Helpers the CLI started (e.g. MCP servers) must not outlive the turn.
    kill_group(process_group);
    if finished {
        return;
    }
    let stderr = match stderr {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    };
    on_event(failure(exit_message(status.ok(), &stderr)));
}

fn spawn(turn: &CliTurn) -> std::io::Result<Child> {
    Command::new(&turn.program)
        .args(&turn.args)
        .current_dir(&turn.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group, so cancelling also stops everything the CLI spawned.
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
}

fn kill_group(pid: Option<u32>) {
    if let Some(pid) = pid.and_then(|p| libc::pid_t::try_from(p).ok()) {
        // SAFETY: killpg only sends a signal. The group id is our own child's pid,
        // made a group leader by process_group(0). ESRCH (already gone) is fine.
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
    }
}

fn exit_message(status: Option<ExitStatus>, stderr: &str) -> String {
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        let start = stderr
            .char_indices()
            .rev()
            .nth(STDERR_TAIL - 1)
            .map_or(0, |(i, _)| i);
        return stderr[start..].to_string();
    }
    match status {
        Some(s) if s.success() => "The assistant ended unexpectedly without an answer.".into(),
        Some(s) => match (s.code(), s.signal()) {
            (Some(code), _) => format!("The assistant ended unexpectedly (exit code {code})."),
            (_, Some(sig)) => format!("The assistant ended unexpectedly (signal {sig})."),
            _ => "The assistant ended unexpectedly.".into(),
        },
        None => "The assistant ended unexpectedly.".into(),
    }
}

fn failure(message: String) -> ChatEvent {
    ChatEvent::Finished {
        is_error: true,
        message: Some(message),
    }
}
