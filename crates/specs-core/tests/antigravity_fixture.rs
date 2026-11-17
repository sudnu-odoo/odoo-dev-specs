//! Replays real recorded `agy -p --output-format stream-json` runs.

use serde_json::json;
use specs_core::antigravity::parse_line;
use specs_core::events::ChatEvent;

fn replay(lines: &str) -> Vec<ChatEvent> {
    lines.lines().flat_map(parse_line).collect()
}

fn text_of(events: &[ChatEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            ChatEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn recorded_plain_answer() {
    let events = replay(include_str!("fixtures/agy_answer.jsonl"));
    assert_eq!(
        events.first(),
        Some(&ChatEvent::Conversation {
            id: "477b1bb0-3c48-4ffc-9992-992a22c2142e".into()
        })
    );
    assert_eq!(text_of(&events), "Hello there, friend!\n");
    assert_eq!(
        events[events.len() - 2..],
        [
            ChatEvent::Usage {
                input_tokens: 9538,
                output_tokens: 183
            },
            ChatEvent::Finished {
                is_error: false,
                message: None
            },
        ]
    );
}

#[test]
fn recorded_run_with_one_tool_call() {
    let events = replay(include_str!("fixtures/agy_read_tool.jsonl"));
    let id = "2b27c3b2-dade-4994-93db-adb0dfdb7252:2".to_string();
    let started = ChatEvent::ToolStarted {
        id: id.clone(),
        name: "read_file".into(),
        input: json!({"Path": "/work/note.txt"}),
    };
    let finished = ChatEvent::ToolFinished {
        id,
        is_error: false,
    };
    let at = |e: &ChatEvent| events.iter().position(|x| x == e).expect("event present");
    assert!(at(&started) < at(&finished));
    assert!(text_of(&events).contains("hello"));
    // Cache reads count as input, like Claude's usage.
    assert!(events.contains(&ChatEvent::Usage {
        input_tokens: 12050 + 7492,
        output_tokens: 698
    }));
    assert_eq!(
        events.last(),
        Some(&ChatEvent::Finished {
            is_error: false,
            message: None
        })
    );
}

#[test]
fn recorded_failure_carries_the_message() {
    let events = replay(include_str!("fixtures/agy_bad_model.jsonl"));
    let Some(ChatEvent::Finished {
        is_error: true,
        message: Some(message),
    }) = events.last()
    else {
        panic!("expected a failed Finished, got {events:?}");
    };
    assert!(message.contains("nonsense-model"), "{message}");
}
