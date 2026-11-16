//! `specs-mcp --session-dir <folder> [--odev <path>]`: MCP server over stdio for one
//! session. With `--odev`, the assistant can also take screenshots of a local Odoo.

use anyhow::{bail, Context, Result};
use odev_core::server::Odev;
use rmcp::ServiceExt;
use specs_mcp::DiagramServer;

const USAGE: &str = "usage: specs-mcp --session-dir <folder> [--odev <path>]";

#[tokio::main]
async fn main() -> Result<()> {
    let (mut session_dir, mut odev) = (None, None);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().context(USAGE)?;
        match flag.as_str() {
            "--session-dir" => session_dir = Some(value),
            "--odev" => odev = Some(value),
            _ => bail!("unknown option {flag}\n{USAGE}"),
        }
    }
    let mut server = DiagramServer::new(session_dir.context(USAGE)?);
    if let Some(odev) = odev {
        // Same state folder as the app, which stops these servers when it closes.
        server = server.with_odoo(odev, Odev::default_state_dir());
    }
    server
        .serve(rmcp::transport::stdio())
        .await
        .context("starting the MCP server")?
        .waiting()
        .await?;
    Ok(())
}
