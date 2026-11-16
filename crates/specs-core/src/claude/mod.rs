//! Claude Code adapter: runs `claude -p` headless and reads its stream-json output.

mod command;
mod parse;

pub use command::{args, TurnSpec, ALLOWED_TOOLS, SYSTEM_PROMPT};
pub use parse::parse_line;
