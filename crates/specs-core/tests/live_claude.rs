//! End-to-end against the real, logged-in Claude Code. Costs real tokens, so it is
//! skipped by default. Run with: `make test-live`

use specs_core::chat::{send_message, ClaudeConfig};
use specs_core::events::ChatEvent;
use specs_core::locate::{find_executable, user_bin_dirs};
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::Workspace;
use tokio::sync::oneshot;

fn real_claude() -> ClaudeConfig {
    let home = dirs::home_dir().unwrap();
    let path = std::env::var("PATH").ok();
    let program = find_executable("claude", path.as_deref(), &user_bin_dirs(&home))
        .expect("Claude Code is not installed");
    ClaudeConfig {
        program,
        provider: specs_core::settings::Provider::Claude,
        read_dirs: vec![],
        model: Some("haiku".into()),
        effort: None,
        diagram_tools: None,
        runtime_dir: None,
        demo_data: true,
    }
}

async fn turn(ws: &Workspace, id: &str, claude: &ClaudeConfig, text: &str) -> ChatEvent {
    let (_tx, rx) = oneshot::channel();
    send_message(ws, "P", id, text, claude, |_| {}, rx)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "calls the real Claude Code; run with `make test-live`"]
async fn real_claude_answers_and_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    let claude = real_claude();

    let first = turn(
        &ws,
        &id,
        &claude,
        "Remember the word 'tangerine'. Reply with exactly: ok",
    )
    .await;
    assert_eq!(
        first,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );
    assert!(ws.session("P", &id).unwrap().started);

    let second = turn(
        &ws,
        &id,
        &claude,
        "Which word did I ask you to remember? One word.",
    )
    .await;
    assert_eq!(
        second,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );
    let last = ws.read_transcript("P", &id).unwrap().pop();
    match last {
        Some(TranscriptEntry::Assistant { text }) => {
            assert!(
                text.to_lowercase().contains("tangerine"),
                "resume lost context: {text}"
            )
        }
        other => panic!("expected an answer, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "calls the real Claude Code; run with `make test-live`"]
async fn real_claude_draws_with_the_diagram_tools() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    assert!(mcp.is_file(), "build it first: cargo build -p specs-mcp");
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    let claude = ClaudeConfig {
        diagram_tools: Some(mcp),
        ..real_claude()
    };

    let done = turn(
        &ws,
        &id,
        &claude,
        "Using the diagram tools, add a blue box 'Sales order', a green box 'Delivery', \
         and an arrow from the first to the second labelled 'plans'. Then reply: done",
    )
    .await;
    assert_eq!(
        done,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );

    let scene: serde_json::Value = serde_json::from_str(&ws.read_scene("P", &id).unwrap()).unwrap();
    let live: Vec<&serde_json::Value> = scene["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isDeleted"] != true)
        .collect();
    let count = |kind: &str| live.iter().filter(|e| e["type"] == kind).count();
    assert_eq!((count("rectangle"), count("arrow")), (2, 1), "{scene}");
    let texts: Vec<&str> = live.iter().filter_map(|e| e["text"].as_str()).collect();
    for expected in ["Sales order", "Delivery", "plans"] {
        assert!(texts.contains(&expected), "missing {expected}: {texts:?}");
    }
    let tools: Vec<_> = ws
        .read_transcript("P", &id)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            TranscriptEntry::Tool { name, .. } => Some(name),
            _ => None,
        })
        .collect();
    assert!(
        tools.iter().any(|t| t == "mcp__specs__diagram_add_arrow"),
        "{tools:?}"
    );
}

#[tokio::test]
#[ignore = "calls the real Claude Code, the Odoo runtime and Chrome; run with `make test-live`"]
async fn real_claude_screenshots_a_fresh_odoo_and_places_it() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    let runtime = odoo_runtime::runtime::Runtime::new(
        odoo_runtime::runtime::Runtime::default_root(),
        Default::default(),
    );
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    let claude = ClaudeConfig {
        diagram_tools: Some(mcp),
        runtime_dir: Some(odoo_runtime::runtime::Runtime::default_root()),
        ..real_claude()
    };

    let done = turn(
        &ws,
        &id,
        &claude,
        "Take a screenshot named 'quotations' of the page /odoo/sales on Odoo 19.0 with the module \
         sale_management, place it on the diagram, then reply: done",
    )
    .await;
    runtime.stop_all().unwrap();
    assert_eq!(
        done,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );

    let session_dir = ws.session_dir("P", &id).unwrap();
    let transcript = ws.read_transcript("P", &id).unwrap();
    let png = std::fs::read(session_dir.join("screenshots/quotations.png"))
        .unwrap_or_else(|e| panic!("screenshot not saved ({e}); transcript: {transcript:#?}"));
    assert_eq!(&png[1..4], b"PNG");
    // Kept for a human look: is it the real page, not the login screen?
    let keep =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-fixtures");
    std::fs::create_dir_all(&keep).unwrap();
    std::fs::write(keep.join("live_quotations.png"), &png).unwrap();
    let scene: serde_json::Value = serde_json::from_str(&ws.read_scene("P", &id).unwrap()).unwrap();
    assert!(
        scene["elements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "image"),
        "image placed"
    );
}

#[tokio::test]
#[ignore = "calls the real Claude Code, the Odoo runtime and Chrome; run with `make test-live`"]
async fn real_claude_starts_a_screen_change_from_a_screenshot() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    let runtime = odoo_runtime::runtime::Runtime::new(
        odoo_runtime::runtime::Runtime::default_root(),
        Default::default(),
    );
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    // Hosting and version chosen, as the app asks before the first message.
    ws.set_hosting("P", &id, Some("online")).unwrap();
    ws.set_version("P", &id, Some("19.0")).unwrap();
    // The model the business team gets by default, not the cheap one.
    let claude = ClaudeConfig {
        diagram_tools: Some(mcp),
        runtime_dir: Some(odoo_runtime::runtime::Runtime::default_root()),
        // The Odoo source of the session's version, as the app passes it.
        read_dirs: runtime.source_dirs_for("19.0"),
        model: None,
        effort: None,
        ..real_claude()
    };

    let started = std::time::Instant::now();
    let done = turn(
        &ws,
        &id,
        &claude,
        "Add a new button beside the Confirm button of sale orders called \"Revise Quotation\"",
    )
    .await;
    // It asks what the button does first; answer like the business analyst would.
    let done =
        match done {
            ChatEvent::Finished {
                is_error: false, ..
            } => turn(
                &ws,
                &id,
                &claude,
                "1a, cancel the original. 2: only quotations not confirmed. 3: all salespeople. \
4: a Revisions smart button, no PDF change. 5: customers ask for changes after the quote. \
6: no past task. Please draw the spec.",
            )
            .await,
            other => other,
        };
    let took = started.elapsed();
    runtime.stop_all().unwrap();
    assert_eq!(
        done,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );

    let tools: Vec<String> = ws
        .read_transcript("P", &id)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            TranscriptEntry::Tool { name, is_error } => {
                Some(format!("{name}{}", if is_error { " (error)" } else { "" }))
            }
            _ => None,
        })
        .collect();
    eprintln!("took {took:?}; tools: {tools:?}");
    // The whole spec in one plan, not dozens of drawing calls.
    assert!(
        tools.iter().any(|t| t == "mcp__specs__spec_build"),
        "{tools:?}"
    );
    let drawing = tools
        .iter()
        .filter(|t| t.starts_with("mcp__specs__diagram_"))
        .count();
    assert!(drawing <= 5, "{drawing} drawing calls: {tools:?}");
    let scene: serde_json::Value = serde_json::from_str(&ws.read_scene("P", &id).unwrap()).unwrap();
    assert!(scene["elements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["type"] == "image" && e["isDeleted"] != true));
    // Keep the result for a human look.
    let keep =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-fixtures");
    std::fs::create_dir_all(&keep).unwrap();
    std::fs::write(
        keep.join("live_revise_quotation.excalidraw"),
        ws.read_scene("P", &id).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
#[ignore = "calls the real Claude Code; run with `make test-live`"]
async fn real_claude_reports_plan_usage_to_the_ui() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    let mut events = Vec::new();
    let (_tx, rx) = oneshot::channel();
    send_message(
        &ws,
        "P",
        &id,
        "Reply with exactly: ok",
        &real_claude(),
        |e| events.push(e),
        rx,
    )
    .await
    .unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            ChatEvent::PlanUsage {
                five_hour: Some(_),
                ..
            }
        )),
        "no plan usage in {events:?}"
    );
}

/// A second kind of request: a report change and two business flows (sales and
/// delivery), where Odoo already has the field.
#[tokio::test]
#[ignore = "calls the real Claude Code; run with `make test-live`"]
async fn real_claude_specs_a_report_change_over_two_flows() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    let runtime = odoo_runtime::runtime::Runtime::new(
        odoo_runtime::runtime::Runtime::default_root(),
        Default::default(),
    );
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    ws.set_hosting("P", &id, Some("online")).unwrap();
    ws.set_version("P", &id, Some("19.0")).unwrap();
    let claude = ClaudeConfig {
        diagram_tools: Some(mcp),
        runtime_dir: Some(odoo_runtime::runtime::Runtime::default_root()),
        read_dirs: runtime.source_dirs_for("19.0"),
        model: None,
        effort: None,
        ..real_claude()
    };

    let started = std::time::Instant::now();
    let done = turn(
        &ws,
        &id,
        &claude,
        "Our customers send a PO number with each order. Salespeople must enter it on the \
quotation before confirming, it must be printed on the quotation PDF, and the warehouse \
wants it printed on the delivery slip too.",
    )
    .await;
    let built = |ws: &Workspace| {
        ws.read_transcript("P", &id).unwrap().iter().any(
            |e| matches!(e, TranscriptEntry::Tool { name, .. } if name.ends_with("spec_build")),
        )
    };
    let done = match done {
        ChatEvent::Finished {
            is_error: false, ..
        } if !built(&ws) => {
            turn(
                &ws,
                &id,
                &claude,
                "Required for all customers. Under the order number on both PDFs. \
All salespeople. No past task. Please draw the spec.",
            )
            .await
        }
        other => other,
    };
    let took = started.elapsed();
    runtime.stop_all().unwrap();
    assert_eq!(
        done,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );
    let tools: Vec<String> = ws
        .read_transcript("P", &id)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            TranscriptEntry::Tool { name, is_error } => {
                Some(format!("{name}{}", if is_error { " (error)" } else { "" }))
            }
            _ => None,
        })
        .collect();
    eprintln!("took {took:?}; tools: {tools:?}");
    assert!(built(&ws), "{tools:?}");
    let keep =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-fixtures");
    std::fs::create_dir_all(&keep).unwrap();
    std::fs::write(
        keep.join("live_po_number.excalidraw"),
        ws.read_scene("P", &id).unwrap(),
    )
    .unwrap();
}

/// A client's quotation PDF attached: the assistant maps it onto Odoo's standard
/// quotation report, from the cleaned text and the layout picture.
#[tokio::test]
#[ignore = "calls the real Claude Code; run with `make test-live`"]
async fn real_claude_maps_an_attached_client_report() {
    let mcp =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/specs-mcp");
    let runtime = odoo_runtime::runtime::Runtime::new(
        odoo_runtime::runtime::Runtime::default_root(),
        Default::default(),
    );
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::new(dir.path());
    ws.create_project("P").unwrap();
    let id = ws.create_session("P", "live", "claude").unwrap().id;
    ws.set_hosting("P", &id, Some("online")).unwrap();
    ws.set_version("P", &id, Some("19.0")).unwrap();
    let claude = ClaudeConfig {
        diagram_tools: Some(mcp),
        runtime_dir: Some(odoo_runtime::runtime::Runtime::default_root()),
        read_dirs: runtime.source_dirs_for("19.0"),
        model: None,
        effort: None,
        ..real_claude()
    };
    let pdf = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/client-quotation.pdf");
    let attachment = specs_core::attachments::attach_pdf(
        &ws.session_dir("P", &id).unwrap(),
        &pdf,
        &Default::default(),
    )
    .unwrap();
    let text = format!(
        "The client wants their quotation PDF to look like this one. Map it to Odoo's standard quotation report and draw the spec.\n\n{}",
        specs_core::attachments::mention(&[attachment])
    );

    let started = std::time::Instant::now();
    let done = turn(&ws, &id, &claude, &text).await;
    let built = |ws: &Workspace| {
        ws.read_transcript("P", &id).unwrap().iter().any(
            |e| matches!(e, TranscriptEntry::Tool { name, .. } if name.ends_with("spec_build")),
        )
    };
    let done = match done {
        ChatEvent::Finished {
            is_error: false, ..
        } if !built(&ws) => turn(
            &ws,
            &id,
            &claude,
            "Standard Odoo where possible, all salespeople, no past task. Please draw the spec.",
        )
        .await,
        other => other,
    };
    let took = started.elapsed();
    runtime.stop_all().unwrap();
    assert_eq!(
        done,
        ChatEvent::Finished {
            is_error: false,
            message: None
        }
    );
    let tools: Vec<String> = ws
        .read_transcript("P", &id)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            TranscriptEntry::Tool { name, is_error } => {
                Some(format!("{name}{}", if is_error { " (error)" } else { "" }))
            }
            _ => None,
        })
        .collect();
    eprintln!("took {took:?}; tools: {tools:?}");
    assert!(built(&ws), "{tools:?}");
    let keep =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-fixtures");
    std::fs::create_dir_all(&keep).unwrap();
    std::fs::write(
        keep.join("live_client_report.excalidraw"),
        ws.read_scene("P", &id).unwrap(),
    )
    .unwrap();
}
