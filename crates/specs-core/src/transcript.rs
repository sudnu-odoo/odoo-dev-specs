//! Chat history as stored in `transcript.jsonl` and shown when a session is reopened.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::events::ChatEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum TranscriptEntry {
    User { text: String },
    Assistant { text: String },
    Tool { name: String, is_error: bool },
    Error { message: String },
}

/// Folds the events of one assistant turn into transcript entries.
#[derive(Debug, Default)]
pub struct TurnRecorder {
    entries: Vec<TranscriptEntry>,
    /// Tool call id -> index of its entry, so parallel results land on the right tool.
    tools: HashMap<String, usize>,
}

impl TurnRecorder {
    pub fn push(&mut self, event: &ChatEvent) {
        match event {
            ChatEvent::TextDelta { text } => match self.entries.last_mut() {
                Some(TranscriptEntry::Assistant { text: current }) => current.push_str(text),
                _ => self
                    .entries
                    .push(TranscriptEntry::Assistant { text: text.clone() }),
            },
            ChatEvent::ToolStarted { id, name, .. } => {
                self.tools.insert(id.clone(), self.entries.len());
                self.entries.push(TranscriptEntry::Tool {
                    name: name.clone(),
                    is_error: false,
                });
            }
            ChatEvent::ToolFinished { id, is_error } => {
                if let Some(TranscriptEntry::Tool { is_error: e, .. }) =
                    self.tools.get(id).and_then(|&i| self.entries.get_mut(i))
                {
                    *e = *is_error;
                }
            }
            ChatEvent::Finished {
                is_error: true,
                message,
            } => {
                let message = message.clone().unwrap_or_else(|| "Unknown error".into());
                self.entries.push(TranscriptEntry::Error { message });
            }
            ChatEvent::Finished { .. } => {}
            // Usage is live-only UI info, not part of the saved conversation.
            ChatEvent::Usage { .. } | ChatEvent::PlanUsage { .. } => {}
        }
    }

    pub fn finish(self) -> Vec<TranscriptEntry> {
        self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use TranscriptEntry as E;

    fn record(events: Vec<ChatEvent>) -> Vec<TranscriptEntry> {
        let mut r = TurnRecorder::default();
        events.iter().for_each(|e| r.push(e));
        r.finish()
    }

    fn text(t: &str) -> ChatEvent {
        ChatEvent::TextDelta { text: t.into() }
    }

    #[test]
    fn deltas_join_into_one_reply() {
        let got = record(vec![
            text("Hel"),
            text("lo"),
            ChatEvent::Finished {
                is_error: false,
                message: None,
            },
        ]);
        assert_eq!(
            got,
            vec![E::Assistant {
                text: "Hello".into()
            }]
        );
    }

    #[test]
    fn tools_split_the_reply_in_order() {
        let got = record(vec![
            text("Let me look."),
            ChatEvent::ToolStarted {
                id: "1".into(),
                name: "Read".into(),
                input: json!({}),
            },
            ChatEvent::ToolFinished {
                id: "1".into(),
                is_error: true,
            },
            text("Done."),
            ChatEvent::Finished {
                is_error: false,
                message: None,
            },
        ]);
        assert_eq!(
            got,
            vec![
                E::Assistant {
                    text: "Let me look.".into()
                },
                E::Tool {
                    name: "Read".into(),
                    is_error: true
                },
                E::Assistant {
                    text: "Done.".into()
                },
            ]
        );
    }

    #[test]
    fn parallel_tool_results_match_by_id() {
        let start = |id: &str| ChatEvent::ToolStarted {
            id: id.into(),
            name: id.into(),
            input: json!({}),
        };
        let got = record(vec![
            start("a"),
            start("b"),
            ChatEvent::ToolFinished {
                id: "a".into(),
                is_error: true,
            },
            ChatEvent::ToolFinished {
                id: "b".into(),
                is_error: false,
            },
        ]);
        assert_eq!(
            got,
            vec![
                E::Tool {
                    name: "a".into(),
                    is_error: true
                },
                E::Tool {
                    name: "b".into(),
                    is_error: false
                }
            ]
        );
    }

    #[test]
    fn failure_is_recorded_after_partial_text() {
        let got = record(vec![
            text("Partial"),
            ChatEvent::Finished {
                is_error: true,
                message: Some("Stopped".into()),
            },
        ]);
        assert_eq!(
            got,
            vec![
                E::Assistant {
                    text: "Partial".into()
                },
                E::Error {
                    message: "Stopped".into()
                }
            ]
        );
    }

    #[test]
    fn unfinished_text_is_kept() {
        assert_eq!(
            record(vec![text("cut")]),
            vec![E::Assistant { text: "cut".into() }]
        );
    }

    #[test]
    fn serialises_with_role_tag() {
        let s = serde_json::to_string(&E::User { text: "hi".into() }).unwrap();
        assert_eq!(s, r#"{"role":"user","text":"hi"}"#);
    }
}
