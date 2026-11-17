//! Provider-neutral chat events streamed to the UI.

use serde::{Deserialize, Serialize};

/// One rate-limit window: fraction used (0..=1) and when it resets (unix seconds).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlanWindow {
    pub utilization: f64,
    pub resets_at: u64,
}

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
    /// Token usage for the turn, reported by the CLI at the end.
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
    /// How much of the subscription's rate-limit windows is used (live-only).
    PlanUsage {
        five_hour: Option<PlanWindow>,
        seven_day: Option<PlanWindow>,
    },
    /// The turn is over. `message` explains the failure when `is_error`.
    Finished {
        is_error: bool,
        message: Option<String>,
    },
}
