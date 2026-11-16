//! Replays a real recorded `gemini -o stream-json` run with a tool call.

use specs_core::events::ChatEvent;
use specs_core::gemini::parse_line;

#[test]
fn recorded_run_with_tool_calls() {
    let events: Vec<ChatEvent> = include_str!("fixtures/gemini_read_tool.jsonl")
        .lines()
        .flat_map(parse_line)
        .collect();

    // Tool calls (update_topic is Gemini's own built-in; both appear at the parse layer),
    // then the assistant's answer, then the finish.
    let starts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            ChatEvent::ToolStarted { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(starts, ["update_topic", "read_file"]);

    assert!(events
        .iter()
        .any(|e| matches!(e, ChatEvent::TextDelta { text } if text.contains("tangerine"))));
    assert_eq!(
        events.last(),
        Some(&ChatEvent::Finished {
            is_error: false,
            message: None
        })
    );

    // Each tool_use is matched by a tool_result.
    let finishes = events
        .iter()
        .filter(|e| matches!(e, ChatEvent::ToolFinished { .. }))
        .count();
    assert_eq!(finishes, 2);
}
