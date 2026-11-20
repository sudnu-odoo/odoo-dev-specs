//! `specs-mcp --session-dir <folder> [--runtime-dir <folder>] [--no-demo-data]`:
//! MCP server over stdio for one session. With `--runtime-dir`, the assistant can
//! also take screenshots of a fresh local Odoo.

use anyhow::Result;
use odoo_runtime::runtime::{Runtime, Tools};
use rmcp::ServiceExt;
use specs_mcp::{parse_args, DiagramServer};

#[tokio::main]
async fn main() -> Result<()> {
    let options = parse_args(std::env::args().skip(1))?;
    let mut server = DiagramServer::new(options.session_dir).with_demo_data(options.demo_data);
    if let Some(root) = options.runtime_dir {
        server = server.with_runtime(Runtime::for_app(root, Tools::default()));
    }
    server
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
