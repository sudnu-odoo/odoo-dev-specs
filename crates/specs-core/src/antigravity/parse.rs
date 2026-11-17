//! Maps one line of `agy --output-format stream-json` to chat events.

use serde_json::Value;

use crate::events::ChatEvent;

/// Parse one stdout line. Lines that are not JSON or carry nothing the UI shows
/// (user echoes, thinking, step bookkeeping) yield no events.
pub fn parse_line(line: &str) -> Vec<ChatEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    match v["event"].as_str() {
        Some("init") => conversation(&v).into_iter().collect(),
        Some("step_update") => step_update(&v["step_update"]).into_iter().collect(),
        Some("result") => result(&v["result"]),
        _ => vec![],
    }
}

fn conversation(init: &Value) -> Option<ChatEvent> {
    let id = init["conversation_id"].as_str().filter(|s| !s.is_empty())?;
    Some(ChatEvent::Conversation { id: id.into() })
}

fn step_update(step: &Value) -> Option<ChatEvent> {
    match step["step_type"].as_str()? {
        "agent_response" => {
            let text = step["text_delta"].as_str().filter(|t| !t.is_empty())?;
            Some(ChatEvent::TextDelta { text: text.into() })
        }
        "tool" => tool(step),
        _ => None,
    }
}

/// A tool step is ACTIVE while running and DONE when it returned. Steps have no
/// id of their own, so conversation + step index identify the call.
fn tool(step: &Value) -> Option<ChatEvent> {
    let id = format!(
        "{}:{}",
        step["conversation_id"].as_str().unwrap_or_default(),
        step["step_index"].as_u64()?
    );
    match step["state"].as_str()? {
        "ACTIVE" => {
            let params = &step["tool_info"]["parameters"];
            // MCP tools all arrive as `call_mcp_tool`; show the real tool, named as Claude does.
            let (name, input) = match (params["ServerName"].as_str(), params["ToolName"].as_str()) {
                (Some(server), Some(tool)) => (
                    format!("mcp__{server}__{tool}"),
                    params["Arguments"].clone(),
                ),
                _ => (
                    step["tool_name"].as_str().unwrap_or("tool").to_string(),
                    params.clone(),
                ),
            };
            Some(ChatEvent::ToolStarted { id, name, input })
        }
        state @ ("DONE" | "ERROR") => Some(ChatEvent::ToolFinished {
            id,
            is_error: state == "ERROR",
        }),
        _ => None,
    }
}

fn result(r: &Value) -> Vec<ChatEvent> {
    let failed = r["status"].as_str() != Some("SUCCESS");
    let message = failed.then(|| {
        r["error"]
            .as_str()
            .filter(|e| !e.is_empty())
            .unwrap_or("Something went wrong.")
            .to_string()
    });
    let u = &r["usage"];
    let n = |key: &str| u[key].as_u64().unwrap_or(0);
    let usage = u.is_object().then(|| ChatEvent::Usage {
        input_tokens: n("input_tokens") + n("cache_read_tokens"),
        output_tokens: n("output_tokens"),
    });
    usage
        .into_iter()
        .chain([ChatEvent::Finished {
            is_error: failed,
            message,
        }])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_ignored() {
        for line in [
            "",
            "not json",
            r#"{"event":"step_update","step_update":{"step_type":"user_input","state":"DONE"}}"#,
            r#"{"event":"step_update","step_update":{"step_type":"agent_response","state":"DONE","text_delta":""}}"#,
            r#"{"event":"something_new"}"#,
        ] {
            assert_eq!(parse_line(line), vec![], "line: {line}");
        }
    }

    const MCP_CALL: &str = r#"{"event":"step_update","step_update":{"conversation_id":"c","step_index":4,"state":"ACTIVE","step_type":"tool","tool_name":"call_mcp_tool","tool_info":{"name":"call_mcp_tool","parameters":{"Arguments":{"a":1},"ServerName":"specs","ToolName":"diagram_summary"}}}}"#;

    #[test]
    fn mcp_calls_are_named_like_claudes() {
        assert_eq!(
            parse_line(MCP_CALL),
            vec![ChatEvent::ToolStarted {
                id: "c:4".into(),
                name: "mcp__specs__diagram_summary".into(),
                input: serde_json::json!({"a": 1}),
            }]
        );
    }

    #[test]
    fn a_failed_tool_step_is_an_error() {
        let line = r#"{"event":"step_update","step_update":{"conversation_id":"c","step_index":4,"state":"ERROR","step_type":"tool","tool_name":"call_mcp_tool","tool_info":{}}}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::ToolFinished {
                id: "c:4".into(),
                is_error: true
            }]
        );
    }

    #[test]
    fn init_without_a_conversation_id_yields_nothing() {
        assert_eq!(
            parse_line(r#"{"event":"init","conversation_id":"","init":{}}"#),
            vec![]
        );
    }
}
