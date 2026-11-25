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
(frames, notes, screenshots, arrows) with the layout problems a reader would see. Score \
each rubric point 0 (missing or wrong), 1 (partly) or 2 (as the reference). Judge the \
decisions and the layout, not the wording; count layout problems against the layout points. \
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
        if std::env::var_os("BENCH_OUTLINE").is_some() {
            println!("{}", outline(&scene));
            continue;
        }
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

/// What is on the canvas, in reading order: each section (a big box or a frame) with
/// its texts and how many screenshots and arrows it holds; and the totals.
fn outline(scene: &Value) -> String {
    let els: Vec<&Value> = scene["elements"]
        .as_array()
        .map(|a| a.iter().filter(|e| e["isDeleted"] != true).collect())
        .unwrap_or_default();
    let num = |e: &Value, k: &str| e[k].as_f64().unwrap_or(0.0);
    let count = |t: &str| els.iter().filter(|e| e["type"] == t).count();
    let red = els
        .iter()
        .filter(|e| e["type"] == "rectangle" && e["strokeColor"].as_str().is_some_and(is_red))
        .count();
    // Sections: frames, or the big boxes the spec tool draws around a requirement.
    let mut sections: Vec<&Value> = els
        .iter()
        .copied()
        .filter(|e| {
            e["type"] == "frame"
                || (e["type"] == "rectangle" && num(e, "width") > 600.0 && num(e, "height") > 250.0)
        })
        .collect();
    sections.sort_by(|a, b| num(a, "y").total_cmp(&num(b, "y")));
    let inside = |e: &Value, s: &Value| {
        let (x, y) = (num(e, "x"), num(e, "y"));
        x >= num(s, "x")
            && y >= num(s, "y")
            && x <= num(s, "x") + num(s, "width")
            && y <= num(s, "y") + num(s, "height")
    };
    let section_of = |e: &Value| sections.iter().rposition(|s| inside(e, s));
    // What a reader trips over: texts covering a screenshot, arrows crossing one.
    let images: Vec<(f64, f64, f64, f64)> = els
        .iter()
        .filter(|e| e["type"] == "image")
        .map(|e| (num(e, "x"), num(e, "y"), num(e, "width"), num(e, "height")))
        .collect();
    let inside = |(x, y): (f64, f64), r: &(f64, f64, f64, f64)| {
        x > r.0 + 2.0 && x < r.0 + r.2 - 2.0 && y > r.1 + 2.0 && y < r.1 + r.3 - 2.0
    };
    let covering = els
        .iter()
        .filter(|e| e["type"] == "text" && e["text"].as_str().is_some_and(|t| t.len() > 3))
        .filter(|e| {
            images.iter().any(|r| {
                num(e, "x") < r.0 + r.2
                    && num(e, "x") + num(e, "width") > r.0
                    && num(e, "y") < r.1 + r.3
                    && num(e, "y") + num(e, "height") > r.1
            })
        })
        .count();
    let crossing = els
        .iter()
        .filter(|e| e["type"] == "arrow")
        .filter(|e| {
            let (x0, y0) = (num(e, "x"), num(e, "y"));
            let pts: Vec<(f64, f64)> = e["points"]
                .as_array()
                .map(|p| {
                    p.iter()
                        .map(|q| {
                            (
                                x0 + q[0].as_f64().unwrap_or(0.0),
                                y0 + q[1].as_f64().unwrap_or(0.0),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let (Some(a), Some(b)) = (pts.first(), pts.last()) else {
                return false;
            };
            images
                .iter()
                .filter(|r| !inside(*a, r) && !inside(*b, r))
                .any(|r| {
                    pts.windows(2).any(|w| {
                        (1..20).any(|i| {
                            let t = i as f64 / 20.0;
                            inside(
                                (
                                    w[0].0 + (w[1].0 - w[0].0) * t,
                                    w[0].1 + (w[1].1 - w[0].1) * t,
                                ),
                                r,
                            )
                        })
                    })
                })
        })
        .count();
    let mut out = format!(
        "Totals: screenshots {}, arrows {}, red outlines {}, sections (boxes or frames) {}\nLayout problems: {covering} texts covering a screenshot, {crossing} arrows crossing a screenshot\n",
        count("image"),
        count("arrow"),
        red,
        sections.len()
    );
    let mut texts: Vec<(Option<usize>, f64, f64, String)> = els
        .iter()
        .filter(|e| e["type"] == "text")
        .map(|e| {
            (
                section_of(e),
                num(e, "y"),
                num(e, "x"),
                e["text"].as_str().unwrap_or("").replace('\n', " / "),
            )
        })
        .collect();
    texts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut current: Option<Option<usize>> = None;
    for (section, _, _, text) in texts {
        if current != Some(section) {
            current = Some(section);
            if let Some(i) = section {
                let holds = |t: &str| {
                    els.iter()
                        .filter(|e| e["type"] == t && section_of(e) == Some(i))
                        .count()
                };
                out.push_str(&format!(
                    "\n## Section box {} ({} screenshots, {} arrows)\n",
                    i + 1,
                    holds("image"),
                    holds("arrow")
                ));
            } else {
                out.push_str("\n## Outside the boxes\n");
            }
        }
        out.push_str(&format!("{text}\n"));
    }
    out
}

fn is_red(c: &str) -> bool {
    let c = c.to_lowercase();
    [
        "#e03131", "#c92a2a", "#ff0000", "#fa5252", "#e8590c", "#f03e3e", "#d00000", "#e00", "red",
    ]
    .iter()
    .any(|r| c.starts_with(r))
}
