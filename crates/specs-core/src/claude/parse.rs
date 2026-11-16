//! Maps one line of `claude --output-format stream-json` to chat events.

use serde_json::Value;

use crate::events::ChatEvent;

/// Parse one stdout line. Lines that are not JSON or carry nothing the UI
/// shows (hooks, status, rate limits, full-message echoes of text) yield no events.
pub fn parse_line(line: &str) -> Vec<ChatEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    match v["type"].as_str() {
        Some("stream_event") => text_delta(&v["event"]).into_iter().collect(),
        Some("assistant") => content_blocks(&v).filter_map(tool_started).collect(),
        Some("user") => content_blocks(&v).filter_map(tool_finished).collect(),
        Some("result") => vec![finished(&v)],
        _ => vec![],
    }
}

fn content_blocks(v: &Value) -> impl Iterator<Item = &Value> {
    v["message"]["content"].as_array().into_iter().flatten()
}

fn text_delta(event: &Value) -> Option<ChatEvent> {
    let delta = &event["delta"];
    (event["type"] == "content_block_delta" && delta["type"] == "text_delta").then(|| {
        ChatEvent::TextDelta {
            text: delta["text"].as_str().unwrap_or_default().to_string(),
        }
    })
}

fn tool_started(block: &Value) -> Option<ChatEvent> {
    (block["type"] == "tool_use").then(|| ChatEvent::ToolStarted {
        id: str_field(block, "id"),
        name: str_field(block, "name"),
        input: block["input"].clone(),
    })
}

fn tool_finished(block: &Value) -> Option<ChatEvent> {
    (block["type"] == "tool_result").then(|| ChatEvent::ToolFinished {
        id: str_field(block, "tool_use_id"),
        is_error: block["is_error"].as_bool().unwrap_or(false),
    })
}

fn finished(v: &Value) -> ChatEvent {
    let is_error = v["is_error"].as_bool().unwrap_or(false) || v["subtype"] != "success";
    let message = is_error.then(|| {
        v["result"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| v["subtype"].as_str())
            .unwrap_or("Unknown error")
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
    fn text_delta_from_partial_stream_event() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hel"}}}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::TextDelta { text: "Hel".into() }]
        );
    }

    #[test]
    fn tool_use_from_assistant_message() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"/a"}}]}}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::ToolStarted {
                id: "t1".into(),
                name: "Read".into(),
                input: json!({"file_path": "/a"})
            }]
        );
    }

    #[test]
    fn assistant_text_is_not_repeated() {
        // Text already arrived through text_delta events.
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"done"}]}}"#;
        assert_eq!(parse_line(line), vec![]);
    }

    #[test]
    fn tool_result_success_and_error() {
        let ok = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"x"}]}}"#;
        let err = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":"boom","is_error":true}]}}"#;
        assert_eq!(
            parse_line(ok),
            vec![ChatEvent::ToolFinished {
                id: "t1".into(),
                is_error: false
            }]
        );
        assert_eq!(
            parse_line(err),
            vec![ChatEvent::ToolFinished {
                id: "t2".into(),
                is_error: true
            }]
        );
    }

    #[test]
    fn successful_result_finishes_turn() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"done"}"#;
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
        let line = r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::Finished {
                is_error: true,
                message: Some("error_max_turns".into())
            }]
        );
        let line =
            r#"{"type":"result","subtype":"success","is_error":true,"result":"Invalid API key"}"#;
        assert_eq!(
            parse_line(line),
            vec![ChatEvent::Finished {
                is_error: true,
                message: Some("Invalid API key".into())
            }]
        );
    }

    #[test]
    fn noise_is_ignored() {
        for line in [
            "",
            "not json",
            r#"{"type":"system","subtype":"init","session_id":"s"}"#,
            r#"{"type":"rate_limit_event"}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{"}}}"#,
        ] {
            assert_eq!(parse_line(line), vec![], "line: {line}");
        }
    }
}
