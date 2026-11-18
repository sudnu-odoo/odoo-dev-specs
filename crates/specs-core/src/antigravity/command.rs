//! Builds the `agy` command line for one chat turn, and its per-session MCP config.

use std::path::{Path, PathBuf};

use crate::claude::{DiagramTools, MCP_SERVER_NAME, SYSTEM_PROMPT};

/// Everything needed to run one turn.
#[derive(Debug, Clone)]
pub struct TurnSpec {
    pub prompt: String,
    /// agy's own id of the conversation to resume; `None` on the first turn.
    pub conversation: Option<String>,
    /// Extra read-only directories, e.g. the Odoo source checkouts.
    pub read_dirs: Vec<PathBuf>,
    /// Model id as listed by `agy models`; `None` uses the CLI default.
    pub model: Option<String>,
    /// Reasoning effort (`low`..`max`); `None` uses the CLI default.
    pub effort: Option<String>,
    /// Whether our diagram tools are offered (their config is written by [`write_mcp_config`]).
    pub diagram_tools: bool,
}

/// Arguments for `agy`. Unlike Claude it takes the prompt as an argument, and has no
/// system-prompt flag, so our instructions lead the first message.
pub fn args(spec: &TurnSpec) -> Vec<String> {
    let prompt = match &spec.conversation {
        None => format!("{SYSTEM_PROMPT}\n\n---\n\n{}", spec.prompt),
        // A leading dash would be read as a flag.
        Some(_) if spec.prompt.starts_with('-') => format!(" {}", spec.prompt),
        Some(_) => spec.prompt.clone(),
    };
    let mut a: Vec<String> = [
        "-p".to_string(),
        prompt,
        "--output-format".into(),
        "stream-json".into(),
    ]
    .into();
    if spec.diagram_tools {
        // Headless runs cannot ask the user: pre-approve every tool of our server.
        let settings = serde_json::json!({ "permissions": { "allow": [format!("mcp({MCP_SERVER_NAME}/*)")] } });
        a.extend(["--settings".to_string(), settings.to_string()]);
    }
    if let Some(id) = &spec.conversation {
        a.extend(["--conversation".to_string(), id.clone()]);
    }
    for dir in &spec.read_dirs {
        a.extend(["--add-dir".to_string(), dir.display().to_string()]);
    }
    if let Some(model) = &spec.model {
        a.extend(["--model".to_string(), model.clone()]);
    }
    // Some models carry their effort in their id (e.g. gemini-3.8-flash-low): agy
    // refuses a second one, so the chosen effort only applies to the others.
    let model_sets_effort = spec
        .model
        .as_deref()
        .is_some_and(|m| ["-low", "-medium", "-high"].iter().any(|s| m.ends_with(s)));
    if let Some(effort) = spec.effort.as_ref().filter(|_| !model_sets_effort) {
        a.extend(["--effort".to_string(), effort.clone()]);
    }
    a
}

/// Write `<session>/.agents/mcp_config.json`, where agy looks for the workspace's MCP servers.
pub fn write_mcp_config(session_dir: &Path, tools: &DiagramTools) -> std::io::Result<()> {
    let dir = session_dir.join(".agents");
    std::fs::create_dir_all(&dir)?;
    let config = serde_json::json!({ "mcpServers": { MCP_SERVER_NAME: {
        "command": tools.program,
        "args": crate::claude::mcp_args(tools),
    }}});
    std::fs::write(dir.join("mcp_config.json"), config.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(conversation: Option<&str>) -> TurnSpec {
        TurnSpec {
            prompt: "Add a field".into(),
            conversation: conversation.map(String::from),
            read_dirs: vec!["/src/odoo".into()],
            model: None,
            effort: None,
            diagram_tools: false,
        }
    }

    fn value_after<'a>(a: &'a [String], flag: &str) -> Option<&'a str> {
        a.iter().position(|x| x == flag).map(|i| a[i + 1].as_str())
    }

    #[test]
    fn first_turn_leads_with_our_instructions_and_starts_a_new_conversation() {
        let a = args(&spec(None));
        assert_eq!(a[0], "-p");
        assert!(a[1].starts_with(SYSTEM_PROMPT));
        assert!(a[1].ends_with("Add a field"));
        assert_eq!(value_after(&a, "--output-format"), Some("stream-json"));
        assert!(!a.contains(&"--conversation".to_string()));
        assert_eq!(value_after(&a, "--add-dir"), Some("/src/odoo"));
    }

    #[test]
    fn later_turns_resume_the_conversation_with_the_bare_message() {
        let a = args(&spec(Some("conv-1")));
        assert_eq!(a[1], "Add a field");
        assert_eq!(value_after(&a, "--conversation"), Some("conv-1"));
    }

    #[test]
    fn a_message_starting_with_a_dash_is_not_taken_for_a_flag() {
        let mut s = spec(Some("c"));
        s.prompt = "-1 day".into();
        assert!(!args(&s)[1].starts_with('-'));
    }

    #[test]
    fn model_is_passed_when_chosen() {
        let mut s = spec(None);
        s.model = Some("claude-sonnet-4-6".into());
        assert_eq!(value_after(&args(&s), "--model"), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn effort_is_passed_when_chosen() {
        let mut s = spec(None);
        s.effort = Some("low".into());
        assert_eq!(value_after(&args(&s), "--effort"), Some("low"));
    }

    #[test]
    fn effort_is_left_out_for_models_whose_name_sets_it() {
        // agy refuses "--model gemini-3.8-flash-low --effort medium".
        for model in [
            "gemini-3.8-flash-low",
            "gemini-3.8-flash-high",
            "gpt-oss-120b-medium",
        ] {
            let mut s = spec(None);
            s.model = Some(model.into());
            s.effort = Some("medium".into());
            assert!(!args(&s).contains(&"--effort".to_string()), "{model}");
        }
        let mut s = spec(None);
        s.model = Some("claude-sonnet-4-6".into());
        s.effort = Some("high".into());
        assert_eq!(value_after(&args(&s), "--effort"), Some("high"));
    }

    #[test]
    fn diagram_tools_are_preapproved_for_this_run_only() {
        let mut s = spec(None);
        s.diagram_tools = true;
        let a = args(&s);
        let settings: serde_json::Value =
            serde_json::from_str(value_after(&a, "--settings").unwrap()).unwrap();
        assert_eq!(
            settings,
            serde_json::json!({"permissions": {"allow": ["mcp(specs/*)"]}})
        );
    }

    #[test]
    fn mcp_config_is_written_where_agy_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        let tools = DiagramTools {
            program: "/app/specs-mcp".into(),
            session_dir: dir.path().into(),
            runtime_dir: None,
            demo_data: true,
        };
        write_mcp_config(dir.path(), &tools).unwrap();
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(".agents/mcp_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(config["mcpServers"]["specs"]["command"], "/app/specs-mcp");
        let args = config["mcpServers"]["specs"]["args"].as_array().unwrap();
        assert_eq!(args[0], "--session-dir");
    }
}
