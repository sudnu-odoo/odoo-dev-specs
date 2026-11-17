//! One chat turn end to end: record the user message, run the AI CLI in the
//! session folder, stream events, and persist the reply.

use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::oneshot;

use crate::claude::{self, DiagramTools, TurnSpec};
use crate::events::ChatEvent;
use crate::runner::{run_turn, CliTurn};
use crate::settings::Provider;
use crate::transcript::{TranscriptEntry, TurnRecorder};
use crate::workspace::Workspace;

/// How to run the AI assistant on this machine. The same shared context (source
/// dirs, diagram tools, Odoo runtime, demo data) is used whichever provider answers.
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    /// The `claude` executable.
    pub program: PathBuf,
    /// Which assistant answers (from the user's settings).
    pub provider: Provider,
    /// Read-only folders for the assistant, e.g. the Odoo source checkouts.
    pub read_dirs: Vec<PathBuf>,
    pub model: Option<String>,
    /// The `specs-mcp` program giving the assistant diagram tools, when found.
    pub diagram_tools: Option<PathBuf>,
    /// Local Odoo runtime folder, for screenshots of a fresh local Odoo.
    pub runtime_dir: Option<PathBuf>,
    /// Load Odoo demo data into screenshot databases (realistic records).
    pub demo_data: bool,
}

/// Send `text` in a session and stream the reply to `on_event`.
///
/// The closing [`ChatEvent::Finished`] is *returned* rather than streamed: it is
/// only produced once the transcript and session state are saved, so the caller
/// can release the session before telling the UI the turn is over.
/// Errors only for workspace problems (unknown session, disk errors); AI failures
/// come back as a failed `Finished` and are saved in the transcript.
pub async fn send_message(
    workspace: &Workspace,
    project: &str,
    session_id: &str,
    text: &str,
    claude: &ClaudeConfig,
    mut on_event: impl FnMut(ChatEvent) + Send,
    cancel: oneshot::Receiver<()>,
) -> Result<ChatEvent> {
    let meta = workspace.session(project, session_id)?;
    let cwd = workspace.session_dir(project, session_id)?;
    workspace.append_transcript(
        project,
        session_id,
        &[TranscriptEntry::User { text: text.into() }],
    )?;

    let spec = TurnSpec {
        session_id: meta.id.clone(),
        resume: meta.started,
        read_dirs: claude.read_dirs.clone(),
        model: claude.model.clone(),
        diagram_tools: claude.diagram_tools.clone().map(|program| DiagramTools {
            program,
            session_dir: cwd.clone(),
            runtime_dir: claude.runtime_dir.clone(),
            demo_data: claude.demo_data,
        }),
    };
    let Invocation {
        program,
        args,
        parse,
    } = match claude.provider {
        Provider::Claude => Invocation {
            program: claude.program.clone(),
            args: claude::args(&spec),
            parse: claude::parse_line,
        },
    };
    let turn = CliTurn {
        program,
        args,
        cwd,
        prompt: text.into(),
    };

    let mut recorder = TurnRecorder::default();
    // The conversation exists once the CLI produced anything, or finished cleanly.
    let mut conversation_exists = false;
    let mut finished = None;
    run_turn(
        turn,
        parse,
        |event| {
            conversation_exists |= !matches!(event, ChatEvent::Finished { is_error: true, .. });
            recorder.push(&event);
            match event {
                ChatEvent::Finished { .. } => finished = Some(event),
                _ => on_event(event),
            }
        },
        cancel,
    )
    .await;

    workspace.append_transcript(project, session_id, &recorder.finish())?;
    if conversation_exists && !meta.started {
        workspace.mark_started(project, session_id)?;
    }
    // run_turn guarantees a Finished; the fallback only guards against a broken invariant.
    Ok(finished.unwrap_or(ChatEvent::Finished {
        is_error: true,
        message: Some("No answer.".into()),
    }))
}

/// How to run the chosen CLI for one turn.
struct Invocation {
    program: PathBuf,
    args: Vec<String>,
    parse: fn(&str) -> Vec<ChatEvent>,
}
