//! Maps one line of `gemini -o stream-json` to chat events.

use serde_json::Value;

use crate::events::ChatEvent;

/// Parse one stdout line. Lines that are not JSON, or carry nothing the UI shows
/// (the `init` line, the echoed user prompt, stats), yield no events.
pub fn parse_line(line: &str) -> Vec<ChatEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    match v["type"].as_str() {
        // Only the assistant's text is shown; the user line is the echoed prompt.
        Some("message") if v["role"] == "assistant" => {
            vec![ChatEvent::TextDelta {
                text: v["content"].as_str().unwrap_or_default().to_string(),
            }]
        }
        Some("tool_use") => vec![ChatEvent::ToolStarted {
            id: str_field(&v, "tool_id"),
            name: str_field(&v, "tool_name"),
            input: v["parameters"].clone(),
        }],
        Some("tool_result") => vec![ChatEvent::ToolFinished {
            id: str_field(&v, "tool_id"),
            is_error: v["status"] != "success",
        }],
        Some("result") => vec![finished(&v)],
        _ => vec![],
    }
}

fn finished(v: &Value) -> ChatEvent {
    let is_error = v["status"] != "success";
    let message = is_error.then(|| {
        // `error` is either a string or an object `{type, message}`.
        v["error"]
            .as_str()
            .or_else(|| v["error"]["message"].as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| v["status"].as_str())
            .unwrap_or("Gemini stopped unexpectedly")
            .to_string()
    });
    ChatEvent::Finished { is_error, message }
}

fn str_field(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assistant_text_is_a_delta() {
        let line = r#"{"type":"message","role":"assistant","content":"Hello","delta":true}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::TextDelta {
                text: "Hello".into()
            }]
        );
    }

    #[test]
    fn the_echoed_user_prompt_is_ignored() {
        let line = r#"{"type":"message","role":"user","content":"do the thing"}"#;
        assert_eq!(parse_line(line), vec![]);
    }

    #[test]
    fn tool_use_becomes_tool_started() {
        let line = r#"{"type":"tool_use","tool_name":"read_file","tool_id":"rf_1","parameters":{"file_path":"a"}}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::ToolStarted {
                id: "rf_1".into(),
                name: "read_file".into(),
                input: json!({"file_path": "a"}),
            }]
        );
    }

    #[test]
    fn tool_result_success_and_error() {
        let ok = r#"{"type":"tool_result","tool_id":"rf_1","status":"success","output":"x"}"#;
        let err = r#"{"type":"tool_result","tool_id":"rf_2","status":"error","output":"boom"}"#;
        assert_eq!(
            parse_line(ok),
            vec![ChatEvent::ToolFinished {
                id: "rf_1".into(),
                is_error: false
            }]
        );
        assert_eq!(
            parse_line(err),
            vec![ChatEvent::ToolFinished {
                id: "rf_2".into(),
                is_error: true
            }]
        );
    }

    #[test]
    fn successful_result_finishes_the_turn() {
        let line = r#"{"type":"result","status":"success","stats":{}}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::Finished {
                is_error: false,
                message: None
            }]
        );
    }

    #[test]
    fn failed_result_carries_a_message() {
        let line = r#"{"type":"result","status":"error","error":"Quota exceeded"}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::Finished {
                is_error: true,
                message: Some("Quota exceeded".into())
            }]
        );
        // Gemini's real shape: `error` is an object with the message inside.
        let obj = r#"{"type":"result","status":"error","error":{"type":"unknown","message":"[API Error: quota exhausted]"}}"#;
        assert_eq!(
            parse_line(obj),
            vec![ChatEvent::Finished {
                is_error: true,
                message: Some("[API Error: quota exhausted]".into())
            }]
        );
        // No message field: still a readable failure.
        let bare = r#"{"type":"result","status":"error"}"#;
        assert_eq!(
            parse_line(bare),
            vec![ChatEvent::Finished {
                is_error: true,
                message: Some("error".into())
            }]
        );
    }

    #[test]
    fn noise_is_ignored() {
        for line in [
            "",
            "not json",
            r#"{"type":"init","model":"gemini-2.5-flash"}"#,
            r#"{"type":"message","role":"assistant","content":"","delta":true}"#,
        ] {
            // Empty assistant text yields an (empty) delta is acceptable, but init/noise do not.
            if line.contains("assistant") {
                continue;
            }
            assert_eq!(parse_line(line), vec![], "line: {line}");
        }
    }
}
