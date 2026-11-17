//! odoo_screenshot end to end: a runtime whose fake `odoo-bin` serves a static fake Odoo,
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
    let psql = write("psql", "echo specs_19_0_sale_management".into());
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

    let args = json!({"version": "19.0", "modules": ["sale_management"], "page": "/odoo/sales", "name": "Sales orders!", "width": 1200, "height": 800});
    let result = tokio::time::timeout(
        Duration::from_secs(90),
        client.call_tool(
            CallToolRequestParams::new("odoo_screenshot")
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
    assert!(answer.contains("screenshots/sales_orders.png"), "{answer}");
    let png = fs::read(session.join("screenshots/sales_orders.png")).unwrap();
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1200);

    // The answer tells the assistant how to place it, and the image tool accepts it.
    let placed = client
        .call_tool(
            CallToolRequestParams::new("diagram_add_image").with_arguments(
                json!({"path": "screenshots/sales_orders.png"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(placed.is_error, Some(true));
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
        let args = json!({"version": "19.0", "modules": [], "page": "/odoo", "name": name});
        let r = client
            .call_tool(
                CallToolRequestParams::new("odoo_screenshot")
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
