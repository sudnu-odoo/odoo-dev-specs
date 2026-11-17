//! The server driven by a real MCP client over an in-memory connection.

use std::fs;
use std::path::Path;

use rmcp::model::{CallToolRequestParams, CallToolResult, ClientConfig};
use rmcp::service::RunningService;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{json, Value};
use specs_mcp::{DiagramServer, SCENE_FILE};
use tempfile::TempDir;

#[derive(Clone, Default)]
struct Client;
impl ClientHandler for Client {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::default()
    }
}

const EMPTY: &str = r##"{"type":"excalidraw","version":2,"source":"x","elements":[],"appState":{"viewBackgroundColor":"#ffffff"},"files":{}}"##;

async fn connect(session: &Path) -> RunningService<RoleClient, Client> {
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    let server = DiagramServer::new(session);
    tokio::spawn(async move { server.serve(server_io).await.unwrap().waiting().await });
    Client.serve(client_io).await.unwrap()
}

fn session() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(SCENE_FILE), EMPTY).unwrap();
    fs::create_dir(dir.path().join("screenshots")).unwrap();
    dir
}

async fn call(
    client: &RunningService<RoleClient, Client>,
    tool: &str,
    args: Value,
) -> CallToolResult {
    let params = CallToolRequestParams::new(tool.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    // A crashed tool would otherwise leave the client waiting forever.
    tokio::time::timeout(std::time::Duration::from_secs(10), client.call_tool(params))
        .await
        .unwrap_or_else(|_| panic!("{tool} did not answer within 10s"))
        .unwrap()
}

fn text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.as_text())
        .map(|t| t.text.clone())
        .collect()
}

fn is_error(result: &CallToolResult) -> bool {
    result.is_error == Some(true)
}

fn scene(dir: &TempDir) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.path().join(SCENE_FILE)).unwrap()).unwrap()
}

/// The id quoted in a tool answer like `Added box abc123.`
fn id_in(answer: &str) -> String {
    answer
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches('.')
        .to_string()
}

#[tokio::test]
async fn offers_the_diagram_tools() {
    let dir = session();
    let client = connect(dir.path()).await;
    let mut names: Vec<String> = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "diagram_add_arrow",
            "diagram_add_box",
            "diagram_add_frame",
            "diagram_add_image",
            "diagram_add_text",
            "diagram_delete",
            "diagram_move",
            "diagram_set_text",
            "diagram_summary",
            "odoo_screenshot",
        ]
    );
}

#[tokio::test]
async fn builds_a_diagram_on_disk() {
    let dir = session();
    let client = connect(dir.path()).await;
    let a = call(
        &client,
        "diagram_add_box",
        json!({"label": "Sales order", "color": "blue"}),
    )
    .await;
    assert!(!is_error(&a), "{}", text(&a));
    let b = call(&client, "diagram_add_box", json!({"label": "Delivery"})).await;
    let arrow = call(
        &client,
        "diagram_add_arrow",
        json!({"from_id": id_in(&text(&a)), "to_id": id_in(&text(&b)), "label": "plans"}),
    )
    .await;
    assert!(!is_error(&arrow), "{}", text(&arrow));

    let kinds: Vec<String> = scene(&dir)["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["type"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds.iter().filter(|k| *k == "rectangle").count(), 2);
    assert_eq!(kinds.iter().filter(|k| *k == "arrow").count(), 1);

    let summary = text(&call(&client, "diagram_summary", json!({})).await);
    let listed: Vec<Value> = serde_json::from_str(&summary).unwrap();
    assert!(listed.iter().any(|e| e["text"] == "Sales order"));
    assert!(listed
        .iter()
        .any(|e| e["kind"] == "arrow" && e["text"] == "plans"));
}

#[tokio::test]
async fn text_frames_moves_and_deletes() {
    let dir = session();
    let client = connect(dir.path()).await;
    let frame = id_in(&text(
        &call(
            &client,
            "diagram_add_frame",
            json!({"name": "Form", "width": 600, "height": 400}),
        )
        .await,
    ));
    let boxed = id_in(&text(
        &call(
            &client,
            "diagram_add_box",
            json!({"label": "Field", "x": 20, "y": 20, "frame_id": frame}),
        )
        .await,
    ));
    let title = id_in(&text(
        &call(
            &client,
            "diagram_add_text",
            json!({"text": "Title", "font_size": 32}),
        )
        .await,
    ));
    assert!(!is_error(
        &call(
            &client,
            "diagram_set_text",
            json!({"id": boxed, "text": "Delivery window"})
        )
        .await
    ));
    assert!(!is_error(
        &call(
            &client,
            "diagram_move",
            json!({"id": title, "x": 0, "y": -80})
        )
        .await
    ));
    let deleted = call(&client, "diagram_delete", json!({"ids": [title]})).await;
    assert!(text(&deleted).contains("1"), "{}", text(&deleted));

    let summary: Vec<Value> =
        serde_json::from_str(&text(&call(&client, "diagram_summary", json!({})).await)).unwrap();
    assert_eq!(summary.len(), 2);
    assert!(summary
        .iter()
        .any(|e| e["text"] == "Delivery window" && e["frame"] == frame.as_str()));
}

#[tokio::test]
async fn images_come_from_the_session_folder_only() {
    let dir = session();
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    png.extend(b"IHDR");
    png.extend(800u32.to_be_bytes());
    png.extend(600u32.to_be_bytes());
    png.extend([8, 6, 0, 0, 0]);
    fs::write(dir.path().join("screenshots/form.png"), &png).unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), &png).unwrap();
    let client = connect(dir.path()).await;

    let ok = call(
        &client,
        "diagram_add_image",
        json!({"path": "screenshots/form.png", "width": 400}),
    )
    .await;
    assert!(!is_error(&ok), "{}", text(&ok));
    assert_eq!(scene(&dir)["files"].as_object().unwrap().len(), 1);

    for path in [
        outside.path().to_str().unwrap(),
        "../outside.png",
        "screenshots/missing.png",
    ] {
        let refused = call(&client, "diagram_add_image", json!({ "path": path })).await;
        assert!(is_error(&refused), "accepted {path}");
    }
}

#[tokio::test]
async fn mistakes_come_back_as_tool_errors_the_assistant_can_read() {
    let dir = session();
    let client = connect(dir.path()).await;
    let bad_color = call(
        &client,
        "diagram_add_box",
        json!({"label": "x", "color": "chartreuse"}),
    )
    .await;
    assert!(is_error(&bad_color));
    assert!(
        text(&bad_color).contains("blue"),
        "lists valid colours: {}",
        text(&bad_color)
    );
    let missing = call(
        &client,
        "diagram_add_arrow",
        json!({"from_id": "nope", "to_id": "nope2"}),
    )
    .await;
    assert!(is_error(&missing) && text(&missing).contains("nope"));
    assert_eq!(
        scene(&dir)["elements"],
        json!([]),
        "failed calls change nothing"
    );
}

#[tokio::test]
async fn a_missing_diagram_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let client = connect(dir.path()).await;
    let result = call(&client, "diagram_summary", json!({})).await;
    assert!(is_error(&result));
}

#[tokio::test]
async fn escaped_line_breaks_become_real_ones() {
    let dir = session();
    let client = connect(dir.path()).await;
    // The assistant sometimes sends a backslash and an "n" instead of a line break.
    let boxed = call(
        &client,
        "diagram_add_box",
        json!({"label": "Quotation\\n(status: draft)"}),
    )
    .await;
    let note = call(&client, "diagram_add_text", json!({"text": "a\\nb"})).await;
    assert!(!is_error(&boxed) && !is_error(&note));
    let texts: Vec<String> = scene(&dir)["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["originalText"].as_str().map(String::from))
        .collect();
    assert_eq!(texts, ["Quotation\n(status: draft)", "a\nb"]);
}

#[tokio::test]
async fn screenshots_need_the_runtime() {
    let dir = session();
    let client = connect(dir.path()).await;
    let result = call(
        &client,
        "odoo_screenshot",
        json!({"version": "19.0", "modules": [], "page": "/odoo", "name": "home"}),
    )
    .await;
    assert!(is_error(&result));
    assert!(text(&result).contains("runtime"), "{}", text(&result));
}

#[tokio::test]
async fn box_answers_tell_the_real_size() {
    let dir = session();
    let client = connect(dir.path()).await;
    let answer = text(
        &call(
            &client,
            "diagram_add_box",
            json!({"label": "Confirm", "width": 120, "height": 40}),
        )
        .await,
    );
    assert!(answer.ends_with("(120x40)."), "{answer}");
}
