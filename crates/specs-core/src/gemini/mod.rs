//! Gemini CLI adapter: runs `gemini -p` headless and reads its stream-json output.

mod command;
mod parse;

pub use command::{args, settings_json, DEFAULT_MODEL};
pub use parse::parse_line;
