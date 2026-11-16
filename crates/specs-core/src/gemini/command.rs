//! Builds the `gemini` command line and the per-session `.gemini/settings.json`
//! for one chat turn. Unlike Claude, Gemini reads its MCP servers from a settings
//! file in the working directory rather than a CLI flag.

use serde_json::{json, Value};

use crate::claude::{DiagramTools, TurnSpec, MCP_SERVER_NAME, SYSTEM_PROMPT};

/// Built-in Gemini tools the assistant may use (read-only file access).
/// Everything else is denied by the policy written into the settings file.
pub const ALLOWED_TOOLS: &[&str] = &[
    "read_file",
    "read_many_files",
    "glob",
    "search_file_content",
];

/// Model used when the user has not chosen one. `auto` is avoided: it can pick an
/// endpoint the machine cannot reach.
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";

/// Arguments for `gemini`. Unlike Claude, Gemini takes the prompt as the value of
/// `-p`, not on stdin.
pub fn args(spec: &TurnSpec, prompt: &str) -> Vec<String> {
    let model = spec
        .model
        .clone()
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let mut a = vec![
        "-p".to_string(),
        prompt.to_string(),
        "-o".to_string(),
        "stream-json".to_string(),
        "-m".to_string(),
        model,
        "--skip-trust".to_string(),
    ];
    let session_flag = if spec.resume {
        "--resume"
    } else {
        "--session-id"
    };
    a.extend([session_flag.to_string(), spec.session_id.clone()]);

    if !spec.read_dirs.is_empty() {
        let dirs = spec
            .read_dirs
            .iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join(",");
        a.extend(["--include-directories".to_string(), dirs]);
    }
    if spec.diagram_tools.is_some() {
        a.extend([
            "--allowed-mcp-server-names".to_string(),
            MCP_SERVER_NAME.to_string(),
        ]);
        // Our tools are read-only file access plus the diagram MCP server; a headless
        // run cannot answer an approval prompt, so auto-approve them.
        a.extend(["--approval-mode".to_string(), "yolo".to_string()]);
    }
    a
}

/// Contents of `<session>/.gemini/settings.json`: the MCP server, allowed tools and
/// the system prompt. With no diagram tools, only the read tools and prompt are set.
pub fn settings_json(tools: Option<&DiagramTools>) -> Value {
    let mut core: Vec<String> = ALLOWED_TOOLS.iter().map(|t| t.to_string()).collect();
    let mut servers = json!({});
    if let Some(tools) = tools {
        servers[MCP_SERVER_NAME] = json!({
            "command": tools.program,
            "args": crate::claude::mcp_args(tools),
        });
        // Allow every tool of our server.
        core.push(format!("mcp__{MCP_SERVER_NAME}"));
    }
    json!({
        "mcpServers": servers,
        "tools": { "core": core },
        "systemPrompt": SYSTEM_PROMPT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(resume: bool) -> TurnSpec {
        TurnSpec {
            session_id: "sess-1".into(),
            resume,
            read_dirs: vec!["/src/odoo".into(), "/src/enterprise".into()],
            model: None,
            diagram_tools: None,
        }
    }

    fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].as_str())
    }

    #[test]
    fn headless_streaming_with_a_pinned_model() {
        let a = args(&spec(false), "do the thing");
        // `-p` runs headless and is a bare flag: the prompt itself goes on stdin.
        assert!(a.contains(&"-p".to_string()), "runs headless");
        assert!(
            !a.iter()
                .any(|arg| arg.contains("note.txt") || arg.len() > 60),
            "no prompt text in args"
        );
        assert_eq!(value_after(&a, "-o"), Some("stream-json"));
        assert_eq!(value_after(&a, "-m"), Some(DEFAULT_MODEL));
        assert!(a.contains(&"--skip-trust".to_string()));
    }

    #[test]
    fn first_turn_sets_session_id_later_turns_resume() {
        let first = args(&spec(false), "do the thing");
        assert_eq!(value_after(&first, "--session-id"), Some("sess-1"));
        assert!(!first.contains(&"--resume".to_string()));
        let next = args(&spec(true), "do the thing");
        assert_eq!(value_after(&next, "--resume"), Some("sess-1"));
        assert!(!next.contains(&"--session-id".to_string()));
    }

    #[test]
    fn odoo_source_dirs_are_included() {
        let a = args(&spec(false), "do the thing");
        assert_eq!(
            value_after(&a, "--include-directories"),
            Some("/src/odoo,/src/enterprise")
        );
    }

    #[test]
    fn a_chosen_model_overrides_the_default() {
        let mut s = spec(false);
        s.model = Some("gemini-2.5-pro".into());
        assert_eq!(
            value_after(&args(&s, "do the thing"), "-m"),
            Some("gemini-2.5-pro")
        );
    }

    #[test]
    fn with_diagram_tools_the_mcp_server_is_allowed() {
        let mut s = spec(false);
        s.diagram_tools = Some(DiagramTools {
            program: "/app/specs-mcp".into(),
            session_dir: "/s".into(),
            odev: None,
            demo_data: true,
        });
        let a = args(&s, "do the thing");
        assert_eq!(
            value_after(&a, "--allowed-mcp-server-names"),
            Some(MCP_SERVER_NAME)
        );
        // Read-only + MCP tools auto-approved; the CLI never blocks on a prompt headless.
        assert_eq!(value_after(&a, "--approval-mode"), Some("yolo"));
    }

    #[test]
    fn settings_file_declares_the_server_tools_and_prompt() {
        let tools = DiagramTools {
            program: "/app/specs-mcp".into(),
            session_dir: "/home/u/OdooSpecs/P/s".into(),
            odev: Some("/bin/odev".into()),
            demo_data: false,
        };
        let s = settings_json(Some(&tools));
        assert_eq!(s["mcpServers"]["specs"]["command"], "/app/specs-mcp");
        assert_eq!(
            s["mcpServers"]["specs"]["args"],
            json!([
                "--session-dir",
                "/home/u/OdooSpecs/P/s",
                "--odev",
                "/bin/odev",
                "--no-demo-data"
            ])
        );
        // The read tools plus our server's tools are the only ones allowed.
        let core: Vec<String> = s["tools"]["core"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_string())
            .collect();
        assert!(core.contains(&"read_file".to_string()));
        assert!(core.iter().any(|t| t.contains("specs")));
        assert_eq!(s["systemPrompt"], SYSTEM_PROMPT);
    }

    #[test]
    fn without_tools_the_settings_have_no_server() {
        let s = settings_json(None);
        assert!(s["mcpServers"].as_object().is_none_or(|m| m.is_empty()));
    }
}
