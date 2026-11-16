//! Builds the `claude` command line for one chat turn.

use std::path::PathBuf;

/// Tools the assistant may use: read-only access to the Odoo source and the session.
pub const ALLOWED_TOOLS: &str = "Read,Grep,Glob";

pub const SYSTEM_PROMPT: &str = "You help Odoo business analysts write development specs. \
The Odoo community and enterprise source code is available read-only; read it to ground your \
answers in how Odoo really works. Answer in plain language for a non-developer audience. \
The spec is an Excalidraw diagram the user sees next to the chat. Build and update it with the \
diagram_* tools: call diagram_summary first, use frames for screens or areas, boxes for models, \
fields and steps, arrows for flows, and short labels. Keep the chat answer brief and put the \
substance in the diagram.";

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
    /// The diagram tools server for this session, when available.
    pub diagram_tools: Option<DiagramTools>,
}

/// `specs-mcp` started by Claude Code for one session folder.
#[derive(Debug, Clone)]
pub struct DiagramTools {
    pub program: PathBuf,
    pub session_dir: PathBuf,
}

/// Name of our MCP server; its tools appear to Claude as `mcp__specs__<tool>`.
pub const MCP_SERVER_NAME: &str = "specs";

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
        "--setting-sources",
        "project",
        "--strict-mcp-config",
        "--append-system-prompt",
        SYSTEM_PROMPT,
    ]
    .map(String::from)
    .into();
    match &spec.diagram_tools {
        Some(tools) => {
            let config = serde_json::json!({ "mcpServers": { MCP_SERVER_NAME: {
                "command": tools.program,
                "args": ["--session-dir", tools.session_dir],
            }}});
            a.extend([
                "--mcp-config".to_string(),
                config.to_string(),
                "--allowedTools".to_string(),
                // Pre-approve every tool of our server (headless runs cannot ask the user).
                format!("{ALLOWED_TOOLS},mcp__{MCP_SERVER_NAME}"),
            ]);
        }
        None => a.extend(["--allowedTools".to_string(), ALLOWED_TOOLS.to_string()]),
    }
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
            diagram_tools: None,
        }
    }

    #[test]
    fn diagram_tools_are_offered_and_preapproved_for_the_session() {
        let mut s = spec(false);
        s.diagram_tools = Some(DiagramTools {
            program: "/app/specs-mcp".into(),
            session_dir: "/home/u/OdooSpecs/Acme/abc".into(),
        });
        let a = args(&s);
        let config: serde_json::Value =
            serde_json::from_str(value_after(&a, "--mcp-config").unwrap()).unwrap();
        assert_eq!(
            config,
            serde_json::json!({"mcpServers": {"specs": {
                "command": "/app/specs-mcp",
                "args": ["--session-dir", "/home/u/OdooSpecs/Acme/abc"],
            }}})
        );
        assert_eq!(
            value_after(&a, "--allowedTools"),
            Some("Read,Grep,Glob,mcp__specs")
        );
        assert_eq!(
            value_after(&a, "--tools"),
            Some(ALLOWED_TOOLS),
            "built-in tools unchanged"
        );
        assert!(
            SYSTEM_PROMPT.contains("diagram_"),
            "the prompt explains the diagram tools"
        );
    }

    #[test]
    fn without_diagram_tools_no_mcp_server_is_configured() {
        let a = args(&spec(false));
        assert_eq!(value_after(&a, "--mcp-config"), None);
        assert_eq!(value_after(&a, "--allowedTools"), Some(ALLOWED_TOOLS));
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
