//! Claude Code adapter: runs `claude -p` headless and reads its stream-json output.

mod command;
mod parse;

pub(crate) use command::mcp_args;
pub use command::{args, DiagramTools, TurnSpec, ALLOWED_TOOLS, MCP_SERVER_NAME, SYSTEM_PROMPT};
pub use parse::parse_line;
