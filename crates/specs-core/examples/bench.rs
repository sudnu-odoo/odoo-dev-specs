//! Test bench: runs example requests through the app's real pipeline (prompt, guides,
//! example cards, Odoo source, diagram tools) and keeps the specs it makes, with how
//! long each took and how many tokens it used, so changes can be compared.
//!
//! ```text
//! cargo build -p specs-mcp
//! cargo run -p specs-core --example bench -- ~/odoo-specs-bench/cases ~/odoo-specs-bench/runs/<name> [one case]
//! ```
//! Each case is a folder holding `case.json`:
//! `{"prompt": "…", "hosting": "sh" | "online", "version": "19.0"}`
//! (and a `rubric.md` for the judge). Cases stay outside the repository: they come
//! from client projects. The judge (`bench_judge`) then scores each result.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use odoo_runtime::runtime::{Runtime, Tools};
use serde::Deserialize;
use serde_json::json;
use specs_core::chat::{send_message, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::settings::Provider;
use specs_core::workspace::Workspace;

#[derive(Deserialize)]
struct Case {
    prompt: String,
    hosting: String,
    version: String,
}

/// Said once when the assistant asks instead of drawing.
const GO_AHEAD: &str = "Go ahead with your recommendation and draw the spec.";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cases, out, only) = match args.as_slice() {
        [cases, out] => (cases, out, None),
        [cases, out, only] => (cases, out, Some(only.as_str())),
        _ => bail!("usage: bench <cases folder> <output folder> [one case]"),
    };
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out)?;
    let workspace = Workspace::new(out.join("workspace"));
    workspace.create_project("bench").ok();
    let mcp = std::env::current_exe()?
        .parent()
        .and_then(Path::parent)
        .map(|debug| debug.join("specs-mcp"))
        .filter(|p| p.is_file())
        .context("Build the diagram tools first: cargo build -p specs-mcp")?;
    let runtime_root = Runtime::default_root();
    let runtime = Runtime::for_app(runtime_root.clone(), Tools::default());

    let mut names: Vec<_> = std::fs::read_dir(cases)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("case.json").is_file())
        .filter(|e| only.is_none_or(|o| e.file_name() == o))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in names {
        let case: Case = serde_json::from_str(&std::fs::read_to_string(
            Path::new(cases).join(&name).join("case.json"),
        )?)
        .with_context(|| format!("reading {name}/case.json"))?;
        println!("── {name}");
        let session = workspace.create_session("bench", &name, "claude")?;
        workspace.set_hosting("bench", &session.id, Some(&case.hosting))?;
        workspace.set_version("bench", &session.id, Some(&case.version))?;
        let config = ClaudeConfig {
            program: which("claude")?,
            provider: Provider::Claude,
            read_dirs: runtime.source_dirs_for(&case.version),
            model: None,
            effort: None,
            diagram_tools: Some(mcp.clone()),
            runtime_dir: Some(runtime_root.clone()),
            demo_data: true,
        };
        let started = Instant::now();
        let (mut input, mut output, mut turns, mut failed) = (0u64, 0u64, 0, None);
        for text in [case.prompt.as_str(), GO_AHEAD] {
            turns += 1;
            let mut on_event = |e: ChatEvent| {
                if let ChatEvent::Usage {
                    input_tokens,
                    output_tokens,
                } = e
                {
                    input += input_tokens;
                    output += output_tokens;
                }
            };
            let (_stop, cancel) = tokio::sync::oneshot::channel();
            let end = send_message(
                &workspace,
                "bench",
                &session.id,
                text,
                &config,
                &mut on_event,
                cancel,
            )
            .await?;
            if let ChatEvent::Finished {
                is_error: true,
                message,
            } = end
            {
                failed = message.or(Some("failed".into()));
                break;
            }
            if drawn(&workspace.read_scene("bench", &session.id)?) {
                break;
            }
        }
        let scene = workspace.read_scene("bench", &session.id)?;
        // How many steps of each kind it took: every step resends the conversation.
        let mut tools = std::collections::BTreeMap::<String, u32>::new();
        for e in workspace.read_transcript("bench", &session.id)? {
            if let specs_core::transcript::TranscriptEntry::Tool { name, .. } = e {
                *tools
                    .entry(name.rsplit("__").next().unwrap_or(&name).to_string())
                    .or_default() += 1;
            }
        }
        std::fs::write(out.join(format!("{name}.excalidraw")), &scene)?;
        let result = json!({
            "case": name,
            "seconds": started.elapsed().as_secs(),
            "turns": turns,
            "input_tokens": input,
            "output_tokens": output,
            "drawn": drawn(&scene),
            "steps": tools.values().sum::<u32>(),
            "tools": tools,
            "error": failed,
        });
        println!("{result}");
        std::fs::write(
            out.join(format!("{name}.json")),
            serde_json::to_string_pretty(&result)?,
        )?;
    }
    Ok(())
}

/// The diagram has something on it.
fn drawn(scene: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(scene)
        .ok()
        .and_then(|v| v["elements"].as_array().map(|e| !e.is_empty()))
        .unwrap_or(false)
}

fn which(program: &str) -> Result<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|dir| dir.join(program))
        .find(|p| p.is_file())
        .with_context(|| format!("{program} is not installed"))
}
