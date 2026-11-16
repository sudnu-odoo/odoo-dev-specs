//! Builds the `claude` command line for one chat turn.

use std::path::PathBuf;

/// Tools the assistant may use: read-only access to the Odoo source and the session.
pub const ALLOWED_TOOLS: &str = "Read,Grep,Glob";

pub const SYSTEM_PROMPT: &str = "You help Odoo business analysts write development specs. \
The Odoo community and enterprise source code is available read-only; read it to ground your \
answers in how Odoo really works. Answer in plain language for a non-developer audience.";

/// Everything needed to run one turn of a session.
#[derive(Debug, Clone)]
pub struct TurnSpec {
    /// Our session id, reused as Claude Code's session id.
    pub session_id: String,
    /// False for the first turn (creates the Claude session), true afterwards (resumes it).
    pub resume: bool,
    /// Extra read-only directories, e.g. the Odoo source checkouts.
    pub read_dirs: Vec<PathBuf>,
    /// Model alias or id; `None` uses the CLI default.
    pub model: Option<String>,
}

/// Arguments for `claude`. The prompt itself is sent on stdin.
pub fn args(spec: &TurnSpec) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--tools",
        ALLOWED_TOOLS,
        "--allowedTools",
        ALLOWED_TOOLS,
        "--setting-sources",
        "project",
        "--strict-mcp-config",
        "--append-system-prompt",
        SYSTEM_PROMPT,
    ]
    .map(String::from)
    .into();
    let session_flag = if spec.resume {
        "--resume"
    } else {
        "--session-id"
    };
    a.extend([session_flag.to_string(), spec.session_id.clone()]);
    for dir in &spec.read_dirs {
        a.extend(["--add-dir".to_string(), dir.display().to_string()]);
    }
    if let Some(model) = &spec.model {
        a.extend(["--model".to_string(), model.clone()]);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(resume: bool) -> TurnSpec {
        TurnSpec {
            session_id: "abc".into(),
            resume,
            read_dirs: vec!["/src/odoo".into(), "/src/enterprise".into()],
            model: None,
        }
    }

    fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].as_str())
    }

    #[test]
    fn headless_streaming_output() {
        let a = args(&spec(false));
        assert_eq!(a[0], "-p");
        assert_eq!(value_after(&a, "--output-format"), Some("stream-json"));
        assert!(a.contains(&"--verbose".to_string()));
        assert!(a.contains(&"--include-partial-messages".to_string()));
    }

    #[test]
    fn locked_down_to_read_only_tools_and_no_user_settings() {
        let a = args(&spec(false));
        assert_eq!(value_after(&a, "--tools"), Some(ALLOWED_TOOLS));
        assert_eq!(value_after(&a, "--allowedTools"), Some(ALLOWED_TOOLS));
        assert_eq!(value_after(&a, "--setting-sources"), Some("project"));
        assert!(a.contains(&"--strict-mcp-config".to_string()));
        assert_eq!(
            value_after(&a, "--append-system-prompt"),
            Some(SYSTEM_PROMPT)
        );
    }

    #[test]
    fn first_turn_sets_session_id_later_turns_resume() {
        let first = args(&spec(false));
        assert_eq!(value_after(&first, "--session-id"), Some("abc"));
        assert!(!first.contains(&"--resume".to_string()));
        let next = args(&spec(true));
        assert_eq!(value_after(&next, "--resume"), Some("abc"));
        assert!(!next.contains(&"--session-id".to_string()));
    }

    #[test]
    fn each_read_dir_is_added() {
        let a = args(&spec(false));
        let dirs: Vec<_> = a
            .windows(2)
            .filter(|w| w[0] == "--add-dir")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(dirs, ["/src/odoo", "/src/enterprise"]);
    }

    #[test]
    fn model_only_when_chosen() {
        assert_eq!(value_after(&args(&spec(false)), "--model"), None);
        let mut s = spec(false);
        s.model = Some("sonnet".into());
        assert_eq!(value_after(&args(&s), "--model"), Some("sonnet"));
    }
}
