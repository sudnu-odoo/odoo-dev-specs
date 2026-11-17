//! Antigravity CLI (`agy`) adapter: runs `agy -p` headless and reads its stream-json output.

mod command;
mod parse;

pub use command::{args, write_mcp_config, TurnSpec};
pub use parse::parse_line;
