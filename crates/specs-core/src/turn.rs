//! Preparing one chat turn: which assistant answers, with which model and effort,
//! and which Odoo source it reads. The app only gathers the inputs and runs the
//! result, so these rules are tested here, without the desktop shell.

use anyhow::{bail, Context, Result};
use odoo_runtime::runtime::{Runtime, Tools};

use crate::chat::ClaudeConfig;
use crate::models;
use crate::providers::ProviderStatus;
use crate::settings::{Provider, Settings};
use crate::workspace::SessionMeta;

/// What the person sent, and the session it goes to.
pub struct TurnRequest<'a> {
    pub session: &'a SessionMeta,
    pub text: &'a str,
    /// The model chosen in the composer for this message.
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// A turn ready to run.
#[derive(Debug)]
pub struct PreparedTurn {
    pub config: ClaudeConfig,
    /// Set when the session gets its assistant with this first message.
    pub new_provider: Option<Provider>,
}

/// Decide how a turn runs, from the app's base config, the latest settings and the
/// assistants on this computer.
///
/// # Errors
/// A message for the person: an empty message, an assistant not installed or not
/// logged in, or one that does not match the session.
pub fn prepare_turn(
    base: &ClaudeConfig,
    settings: &Settings,
    assistants: &[ProviderStatus],
    request: TurnRequest<'_>,
) -> Result<PreparedTurn> {
    if request.text.trim().is_empty() {
        bail!("The message is empty.");
    }
    let session = request.session;
    let mut config = base.clone();
    // The latest settings, so changing demo data takes effect without a restart.
    config.demo_data = settings.demo_data;
    // A session belongs to the assistant of its first message; the chosen model
    // decides only for a new session.
    config.provider = models::session_provider(
        &session.provider,
        session.started,
        request.model.as_deref().and_then(models::provider_of),
        settings.provider,
    )
    .map_err(anyhow::Error::msg)?;
    let status = assistants
        .iter()
        .find(|s| s.provider == config.provider)
        .context("Unknown assistant.")?;
    config.program = match &status.program {
        Some(program) if status.logged_in => program.clone(),
        Some(_) => bail!(
            "{} is not logged in. Log in from the model menu or in Settings.",
            status.name
        ),
        None => bail!("{} is not installed on this computer.", status.name),
    };
    config.effort = models::valid_effort(request.effort);
    config.model = request
        .model
        .filter(|m| models::is_valid(config.provider, m));
    if let Some(root) = &config.runtime_dir {
        config.read_dirs = source_dirs(root, session.version.as_deref());
    }
    let new_provider =
        (!session.started && session.provider != config.provider.id()).then_some(config.provider);
    Ok(PreparedTurn {
        config,
        new_provider,
    })
}

/// The Odoo source the assistant reads: only the session's version when it is here,
/// so it never mixes up two versions' code; otherwise every version downloaded.
fn source_dirs(runtime_root: &std::path::Path, version: Option<&str>) -> Vec<std::path::PathBuf> {
    let odoo = Runtime::new(runtime_root, Tools::default());
    let own = version.map(|v| odoo.source_dirs_for(v)).unwrap_or_default();
    if own.is_empty() {
        odoo.source_dirs()
    } else {
        own
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn base() -> ClaudeConfig {
        ClaudeConfig {
            program: PathBuf::new(),
            provider: Provider::Claude,
            read_dirs: vec![],
            model: None,
            effort: None,
            diagram_tools: None,
            runtime_dir: None,
            demo_data: false,
        }
    }

    fn assistant(provider: Provider, installed: bool, logged_in: bool) -> ProviderStatus {
        ProviderStatus {
            provider,
            name: provider.display_name().into(),
            installed,
            logged_in,
            program: installed.then(|| PathBuf::from(format!("/bin/{}", provider.id()))),
        }
    }

    fn session(provider: &str, started: bool) -> SessionMeta {
        serde_json::from_value(serde_json::json!({
            "id": "s1", "title": "t", "provider": provider,
            "created_at": 0, "updated_at": 0, "started": started
        }))
        .unwrap()
    }

    fn request<'a>(
        session: &'a SessionMeta,
        text: &'a str,
        model: Option<&str>,
    ) -> TurnRequest<'a> {
        TurnRequest {
            session,
            text,
            model: model.map(String::from),
            effort: Some("high".into()),
        }
    }

    fn both() -> Vec<ProviderStatus> {
        vec![
            assistant(Provider::Claude, true, true),
            assistant(Provider::Antigravity, true, true),
        ]
    }

    #[test]
    fn an_empty_message_is_refused() {
        let s = session("claude", false);
        let err = prepare_turn(
            &base(),
            &Settings::default(),
            &both(),
            request(&s, "  ", None),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "The message is empty.");
    }

    #[test]
    fn a_new_session_takes_the_assistant_of_the_chosen_model() {
        let s = session("claude", false);
        let turn = prepare_turn(
            &base(),
            &Settings::default(),
            &both(),
            request(&s, "hi", Some("gemini-3.8-flash-medium")),
        )
        .unwrap();
        assert_eq!(turn.config.provider, Provider::Antigravity);
        assert_eq!(turn.new_provider, Some(Provider::Antigravity));
        assert_eq!(turn.config.program, PathBuf::from("/bin/antigravity"));
        assert_eq!(
            turn.config.model.as_deref(),
            Some("gemini-3.8-flash-medium")
        );
    }

    #[test]
    fn a_started_session_keeps_its_assistant() {
        let s = session("claude", true);
        // A model of another assistant is refused: never switched mid-conversation.
        let err = prepare_turn(
            &base(),
            &Settings::default(),
            &both(),
            request(&s, "hi", Some("gemini-3.8-flash-medium")),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Start a new session"), "{err}");
        let turn = prepare_turn(
            &base(),
            &Settings::default(),
            &both(),
            request(&s, "hi", None),
        )
        .unwrap();
        assert_eq!(turn.config.provider, Provider::Claude);
        assert_eq!(turn.new_provider, None);
    }

    #[test]
    fn an_assistant_must_be_installed_and_logged_in() {
        let s = session("claude", false);
        let logged_out = [assistant(Provider::Claude, true, false)];
        let err = prepare_turn(
            &base(),
            &Settings::default(),
            &logged_out,
            request(&s, "hi", None),
        )
        .unwrap_err();
        assert!(err.to_string().contains("is not logged in"), "{err}");
        let missing = [assistant(Provider::Claude, false, false)];
        let err = prepare_turn(
            &base(),
            &Settings::default(),
            &missing,
            request(&s, "hi", None),
        )
        .unwrap_err();
        assert!(err.to_string().contains("is not installed"), "{err}");
    }

    #[test]
    fn the_latest_settings_and_a_valid_effort_apply() {
        let s = session("claude", true);
        let settings = Settings {
            demo_data: true,
            ..Settings::default()
        };
        let mut req = request(&s, "hi", None);
        req.effort = Some("ludicrous".into());
        let turn = prepare_turn(&base(), &settings, &both(), req).unwrap();
        assert!(turn.config.demo_data);
        assert_eq!(turn.config.effort, None, "an unknown effort is left out");
    }

    #[test]
    fn the_assistant_reads_only_the_sessions_version_when_it_is_here() {
        let root = tempfile::tempdir().unwrap();
        for v in ["19.0", "20.0"] {
            let odoo = root.path().join("sources").join(v).join("odoo");
            std::fs::create_dir_all(&odoo).unwrap();
            std::fs::write(odoo.join("odoo-bin"), "").unwrap();
        }
        let mut config = base();
        config.runtime_dir = Some(root.path().into());
        let mut s = session("claude", true);
        s.version = Some("19.0".into());
        let turn = prepare_turn(
            &config,
            &Settings::default(),
            &both(),
            request(&s, "hi", None),
        )
        .unwrap();
        assert_eq!(turn.config.read_dirs.len(), 1);
        assert!(turn.config.read_dirs[0].ends_with("19.0/odoo"));
        // Not chosen yet: every version here.
        s.version = None;
        let turn = prepare_turn(
            &config,
            &Settings::default(),
            &both(),
            request(&s, "hi", None),
        )
        .unwrap();
        assert_eq!(turn.config.read_dirs.len(), 2);
    }
}
