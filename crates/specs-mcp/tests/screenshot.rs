//! odoo_screenshots end to end: a runtime whose fake `odoo-bin` serves a static fake Odoo,
//! the real server management and real headless Chrome.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use odoo_runtime::runtime::{Runtime, Tools, VENV_READY};
use rmcp::model::{CallToolRequestParams, ClientConfig};
use rmcp::{ClientHandler, ServiceExt};
use serde_json::json;
use specs_mcp::{DiagramServer, SCENE_FILE};

#[derive(Clone, Default)]
struct Client;
impl ClientHandler for Client {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::default()
    }
}

/// A folder served by `python3 -m http.server` that behaves like Odoo's login.
fn fake_odoo_site(root: &std::path::Path) {
    fs::create_dir_all(root.join("web/login")).unwrap();
    fs::create_dir_all(root.join("after")).unwrap();
    fs::write(
        root.join("web/login/index.html"),
        r#"<form action="/after/"><input name="login"><input name="password"><button type="submit">Go</button></form>"#,
    )
    .unwrap();
    fs::write(
        root.join("after/index.html"),
        r#"<body style="background:#714b67"><div class="o_action_manager"><div>Sales</div></div></body>"#,
    )
    .unwrap();
    // Pages the assistant asks for, like Odoo serves them once logged in.
    fs::create_dir_all(root.join("odoo/sales")).unwrap();
    fs::write(
        root.join("odoo/sales/index.html"),
        r#"<body style="margin:0"><div class="o_action_manager"><div>
<table style="position:absolute;left:20px;top:100px"><tr><th data-name="partner_id">Customer</th></tr></table>
</div></div></body>"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("odoo/sales/3")).unwrap();
    fs::write(
        root.join("odoo/sales/3/index.html"),
        r#"<body style="margin:0"><div class="o_action_manager"><div>
<label class="o_form_label" for="validity_date_0" style="position:absolute;left:470px;top:240px">Expiration</label>
<div class="o_field_widget" name="validity_date" style="position:absolute;left:590px;top:236px;width:295px;height:24px">Dec 17</div>
</div></div></body>"#,
    )
    .unwrap();
}

/// A runtime whose Odoo 19.0 is already set up (source + ready virtualenv), so no
/// git or pip runs. `psql` lists one database; `odoo-bin` serves `site` on its port.
fn fake_runtime(work: &std::path::Path, site: &std::path::Path) -> Runtime {
    let bin = work.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let write = |name: &str, body: String| {
        let p = bin.join(name);
        fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    };
    // Lists one database, which has Sales installed.
    let psql = write(
        "psql",
        r#"case "$*" in *ir_module_module*) echo sale_management;; *) echo specs_19_0_sale_management_demo;; esac"#.into(),
    );
    let python = write(
        "python",
        format!(
            r#"port=""; prev=""
for a in "$@"; do [ "$prev" = "--http-port" ] && port="$a"; prev="$a"; done
cd '{}' && exec /usr/bin/python3 -m http.server "$port" --bind 127.0.0.1"#,
            site.display()
        ),
    );
    let runtime = Runtime::new(
        work.join("runtime"),
        Tools {
            git: "/bin/false".into(),
            python: python.clone(),
            psql,
            dropdb: "/bin/true".into(),
        },
    );
    let source = runtime.source_dir("19.0");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("odoo-bin"), "").unwrap();
    let venv_bin = work.join("runtime/venvs/19.0/bin");
    fs::create_dir_all(&venv_bin).unwrap();
    std::os::unix::fs::symlink(&python, venv_bin.join("python")).unwrap();
    fs::write(work.join("runtime/venvs/19.0").join(VENV_READY), "").unwrap();
    runtime
}

#[tokio::test]
async fn takes_a_screenshot_of_a_fresh_local_odoo_into_the_session() {
    let work = tempfile::tempdir().unwrap();
    let site = work.path().join("site");
    fake_odoo_site(&site);
    let runtime = fake_runtime(work.path(), &site);
    let session = work.path().join("session");
    fs::create_dir_all(session.join("screenshots")).unwrap();
    fs::write(
        session.join(SCENE_FILE),
        r#"{"type":"excalidraw","elements":[],"files":{}}"#,
    )
    .unwrap();
    let server = DiagramServer::new(&session).with_runtime(runtime.clone());

    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move { server.serve(server_io).await.unwrap().waiting().await });
    let client = Client.serve(client_io).await.unwrap();

    let args = json!({"version": "19.0", "modules": ["sale_management"], "pages": [
        {"page": "/odoo/sales", "name": "Sales orders!", "width": 1200, "height": 800},
        {"page": "/odoo/sales/3", "name": "quotation_form"}
    ]});
    let result = tokio::time::timeout(
        Duration::from_secs(120),
        client.call_tool(
            CallToolRequestParams::new("odoo_screenshots")
                .with_arguments(args.as_object().cloned().unwrap()),
        ),
    )
    .await
    .expect("timed out")
    .unwrap();
    let answer: String = result
        .content
        .iter()
        .filter_map(|c| c.as_text())
        .map(|t| t.text.clone())
        .collect();
    runtime.stop_all().unwrap();

    assert_ne!(result.is_error, Some(true), "{answer}");
    // In Auto, the session keeps the database its first screenshots used.
    assert_eq!(
        odoo_runtime::runtime::session_database(&session).as_deref(),
        Some("specs_19_0_sale_management_demo")
    );
    assert!(answer.contains("screenshots/sales_orders.png"), "{answer}");
    assert!(
        answer.contains("screenshots/quotation_form.png"),
        "{answer}"
    );
    let png = fs::read(session.join("screenshots/sales_orders.png")).unwrap();
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1200);
    let png = fs::read(session.join("screenshots/quotation_form.png")).unwrap();
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1440);

    // What is on each screenshot is saved next to it and named in the answer.
    let map: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(session.join("screenshots/quotation_form.json")).unwrap(),
    )
    .unwrap();
    assert!(
        map.as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "validity_date"),
        "{map}"
    );
    assert!(answer.contains("validity_date"), "{answer}");
    assert!(answer.contains("column:partner_id"), "{answer}");

    // The image tool accepts it, and anchors find the field on it.
    let call = |tool: &str, args: serde_json::Value| {
        client.call_tool(
            CallToolRequestParams::new(tool.to_string())
                .with_arguments(args.as_object().cloned().unwrap()),
        )
    };
    let placed = call(
        "diagram_add_image",
        json!({"path": "screenshots/quotation_form.png", "x": 0, "y": 0, "width": 720}),
    )
    .await
    .unwrap();
    assert_ne!(placed.is_error, Some(true));
    let image = placed.content[0]
        .as_text()
        .unwrap()
        .text
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches('.')
        .to_string();
    let outlined = call(
        "diagram_add_highlight",
        json!({"image_id": image, "anchor": "validity_date"}),
    )
    .await
    .unwrap();
    let text = outlined.content[0].as_text().unwrap().text.clone();
    assert_ne!(outlined.is_error, Some(true), "{text}");
}

#[tokio::test]
async fn screenshot_names_stay_inside_the_session() {
    let work = tempfile::tempdir().unwrap();
    let missing = Tools {
        git: "/nonexistent/git".into(),
        python: "/nonexistent/python".into(),
        psql: "/nonexistent/psql".into(),
        dropdb: "/nonexistent/dropdb".into(),
    };
    let server = DiagramServer::new(work.path())
        .with_runtime(Runtime::new(work.path().join("runtime"), missing));
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move { server.serve(server_io).await.unwrap().waiting().await });
    let client = Client.serve(client_io).await.unwrap();
    for name in ["../escape", "/etc/passwd", ""] {
        let args =
            json!({"version": "19.0", "modules": [], "pages": [{"page": "/odoo", "name": name}]});
        let r = client
            .call_tool(
                CallToolRequestParams::new("odoo_screenshots")
                    .with_arguments(args.as_object().cloned().unwrap()),
            )
            .await
            .unwrap();
        let answer: String = r
            .content
            .iter()
            .filter_map(|c| c.as_text())
            .map(|t| t.text.clone())
            .collect();
        if name.is_empty() {
            assert_eq!(r.is_error, Some(true), "empty name accepted");
        } else {
            // Sanitised to a plain file name: the failure is about the runtime, not the path.
            assert!(!answer.contains(".."), "{answer}");
        }
    }
}

#[tokio::test]
async fn a_session_with_a_chosen_database_takes_its_screenshots_there() {
    let work = tempfile::tempdir().unwrap();
    let site = work.path().join("site");
    fake_odoo_site(&site);
    let runtime = fake_runtime(work.path(), &site);
    let session = work.path().join("session");
    fs::create_dir_all(&session).unwrap();
    fs::write(
        session.join(SCENE_FILE),
        r#"{"type":"excalidraw","elements":[],"files":{}}"#,
    )
    .unwrap();
    odoo_runtime::runtime::set_session_database(&session, Some("specs_19_0_sale_management_demo"))
        .unwrap();
    let server = DiagramServer::new(&session).with_runtime(runtime.clone());
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move { server.serve(server_io).await.unwrap().waiting().await });
    let client = Client.serve(client_io).await.unwrap();

    // Asked for another version: the session's database decides.
    let args = json!({"version": "18.0", "modules": ["sale_management"], "pages": [
        {"page": "/odoo/sales/3", "name": "quotation_form"}
    ]});
    let result = tokio::time::timeout(
        Duration::from_secs(120),
        client.call_tool(
            CallToolRequestParams::new("odoo_screenshots")
                .with_arguments(args.as_object().cloned().unwrap()),
        ),
    )
    .await
    .expect("timed out")
    .unwrap();
    let answer: String = result
        .content
        .iter()
        .filter_map(|c| c.as_text())
        .map(|t| t.text.clone())
        .collect();
    runtime.stop_all().unwrap();
    assert_ne!(result.is_error, Some(true), "{answer}");
    assert!(answer.contains("Odoo 19.0"), "{answer}");
    assert!(answer.contains("the session's database"), "{answer}");
    assert!(session.join("screenshots/quotation_form.png").exists());
}
