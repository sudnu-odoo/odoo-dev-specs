//! Provider-neutral chat events streamed to the UI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEvent {
    /// A piece of assistant text, to append to the current reply.
    TextDelta { text: String },
    /// The assistant called a tool.
    ToolStarted {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// A tool call returned.
    ToolFinished { id: String, is_error: bool },
    /// The turn is over. `message` explains the failure when `is_error`.
    Finished {
        is_error: bool,
        message: Option<String>,
    },
}
