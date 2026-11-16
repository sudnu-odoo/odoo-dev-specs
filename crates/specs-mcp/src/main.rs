//! `specs-mcp --session-dir <folder>`: MCP server over stdio for one session.

use anyhow::{bail, Context, Result};
use rmcp::ServiceExt;
use specs_mcp::DiagramServer;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let session_dir = match (args.next().as_deref(), args.next()) {
        (Some("--session-dir"), Some(dir)) => dir,
        _ => bail!("usage: specs-mcp --session-dir <folder>"),
    };
    let server = DiagramServer::new(session_dir)
        .serve(rmcp::transport::stdio())
        .await
        .context("starting the MCP server")?;
    server.waiting().await?;
    Ok(())
}
