//! Replays a real recorded `claude -p --output-format stream-json` run.

use serde_json::json;
use specs_core::claude::parse_line;
use specs_core::events::ChatEvent;

#[test]
fn recorded_run_with_one_tool_call() {
    let events: Vec<ChatEvent> = include_str!("fixtures/claude_read_tool.jsonl")
        .lines()
        .flat_map(parse_line)
        .collect();
    let id = "toolu_015H4AjibagAfkbQuxvvA9d2".to_string();
    assert_eq!(
        events,
        vec![
            ChatEvent::ToolStarted {
                id: id.clone(),
                name: "Read".into(),
                input: json!({"file_path": "/work/note.txt"}),
            },
            ChatEvent::ToolFinished {
                id,
                is_error: false
            },
            ChatEvent::TextDelta {
                text: "done".into()
            },
            ChatEvent::Finished {
                is_error: false,
                message: None
            },
        ]
    );
}
