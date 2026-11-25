//! Scores the specs a bench run made, one case at a time, against the case's
//! `rubric.md` (written from the reference spec, without client data). The judge only
//! sees the app's result and the rubric, never the client's reference.
//!
//! ```text
//! cargo run -p specs-core --example bench_judge -- ~/odoo-specs-bench/cases ~/odoo-specs-bench/runs/<name>
//! ```
//! Writes `<case>.score.json` next to each result and a `report.md` for the run.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

const ASK: &str = "You judge an Odoo functional specification drawn on a canvas, against a \
rubric written from a reference spec. Below are the rubric, then the spec's outline \
(frames, notes, screenshots, arrows). Score each rubric point 0 (missing or wrong), 1 \
(partly) or 2 (as the reference). Judge the decisions and the layout, not the wording. \
Answer with JSON only: {\"items\":[{\"point\":\"…\",\"score\":0,\"why\":\"…\"}],\"summary\":\"one sentence\"}";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cases, run] = args.as_slice() else {
        bail!("usage: bench_judge <cases folder> <run folder>");
    };
    let mut rows = vec![
        "| Case | Score | Time | Tokens in / out | Summary |".to_string(),
        "|---|---|---|---|---|".to_string(),
    ];
    let mut names: Vec<String> = std::fs::read_dir(run)?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".excalidraw").map(String::from))
        .collect();
    names.sort();
    for name in names {
        let rubric = std::fs::read_to_string(Path::new(cases).join(&name).join("rubric.md"))
            .with_context(|| format!("{name}: no rubric.md"))?;
        let scene: Value = serde_json::from_str(&std::fs::read_to_string(
            Path::new(run).join(format!("{name}.excalidraw")),
        )?)?;
        let prompt = format!(
            "{ASK}\n\n# Rubric\n{rubric}\n\n# Spec outline\n{}",
            outline(&scene)
        );
        let out = Command::new("claude")
            .args([
                "-p",
                "--setting-sources",
                "project",
                "--strict-mcp-config",
                &prompt,
            ])
            .current_dir(std::env::temp_dir())
            .output()
            .context("running claude")?;
        let text = String::from_utf8_lossy(&out.stdout);
        let json = text
            .find('{')
            .and_then(|a| text.rfind('}').map(|b| &text[a..=b]))
            .context("the judge gave no JSON")?;
        let score: Value = serde_json::from_str(json)?;
        std::fs::write(
            Path::new(run).join(format!("{name}.score.json")),
            serde_json::to_string_pretty(&score)?,
        )?;
        let items = score["items"].as_array().cloned().unwrap_or_default();
        let got: u64 = items.iter().filter_map(|i| i["score"].as_u64()).sum();
        let meta: Value = std::fs::read_to_string(Path::new(run).join(format!("{name}.json")))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        rows.push(format!(
            "| {name} | {got}/{} | {}s | {} / {} | {} |",
            items.len() * 2,
            meta["seconds"],
            meta["input_tokens"],
            meta["output_tokens"],
            score["summary"].as_str().unwrap_or("")
        ));
        println!("{name}: {got}/{}", items.len() * 2);
    }
    std::fs::write(Path::new(run).join("report.md"), rows.join("\n") + "\n")?;
    println!("{}", rows.join("\n"));
    Ok(())
}

/// What is on the canvas, in reading order: frames, then each text with its frame;
/// and how many screenshots, arrows and red outlines there are.
fn outline(scene: &Value) -> String {
    let els: Vec<&Value> = scene["elements"]
        .as_array()
        .map(|a| a.iter().filter(|e| e["isDeleted"] != true).collect())
        .unwrap_or_default();
    let count = |t: &str| els.iter().filter(|e| e["type"] == t).count();
    let red = els
        .iter()
        .filter(|e| {
            e["type"] == "rectangle"
                && e["strokeColor"].as_str().is_some_and(|c| {
                    c.starts_with("#e0")
                        || c == "#ff0000"
                        || c.starts_with("#c92a2a")
                        || c.starts_with("#e03131")
                })
        })
        .count();
    let frame_name = |id: &Value| {
        els.iter()
            .find(|f| f["type"] == "frame" && f["id"] == *id)
            .and_then(|f| f["name"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let mut texts: Vec<(String, f64, f64, String)> = els
        .iter()
        .filter(|e| e["type"] == "text")
        .map(|e| {
            (
                frame_name(&e["frameId"]),
                e["y"].as_f64().unwrap_or(0.0),
                e["x"].as_f64().unwrap_or(0.0),
                e["text"].as_str().unwrap_or("").replace('\n', " / "),
            )
        })
        .collect();
    texts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut s = format!(
        "Screenshots: {}, arrows: {}, red outlines: {}, frames: {}\n",
        count("image"),
        count("arrow"),
        red,
        count("frame")
    );
    for (frame, _, _, text) in texts {
        s.push_str(&format!("[{frame}] {text}\n"));
    }
    s
}
