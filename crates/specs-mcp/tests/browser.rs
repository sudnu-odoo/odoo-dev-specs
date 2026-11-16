//! Real headless Chrome against a tiny fake Odoo served by the test.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use specs_mcp::browser::{screenshot, Viewport};

const LOGIN: &str = r#"<html><body><form action="/after" method="get">
<input name="login"><input name="password" type="password"><button type="submit">Log in</button>
</form></body></html>"#;

/// Like Odoo, the view appears a moment after the page loads.
const AFTER: &str = r#"<html><body style="background:#714b67"><div class="o_action_manager"></div>
<script>setTimeout(() => {
  document.querySelector('.o_action_manager').innerHTML = '<div class="o_view_controller">Sales</div>';
}, 300);</script></body></html>"#;

/// Serves the fake Odoo; records the request lines it received.
fn fake_odoo() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap_or_default();
            log.lock().unwrap().push(request.trim().to_string());
            let body = if request.contains(" /after") {
                AFTER
            } else if request.contains(" /web/login") {
                LOGIN
            } else {
                ""
            };
            let status = if body.is_empty() {
                "404 Not Found"
            } else {
                "200 OK"
            };
            let mut stream = stream;
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (url, seen)
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    let read = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (read(16), read(20))
}

#[tokio::test]
async fn logs_in_waits_for_the_view_and_saves_a_png() {
    let (url, seen) = fake_odoo();
    let out = tempfile::tempdir().unwrap();
    let file = out.path().join("sales.png");

    tokio::time::timeout(
        Duration::from_secs(60),
        screenshot(
            &url,
            "/odoo/sales",
            Viewport {
                width: 1000,
                height: 700,
            },
            &file,
        ),
    )
    .await
    .expect("screenshot timed out")
    .unwrap();

    let png = std::fs::read(&file).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    assert_eq!(png_size(&png), (1000, 700));
    let requests = seen.lock().unwrap().clone();
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("GET /web/login?redirect=%2Fodoo%2Fsales")),
        "{requests:?}"
    );
    assert!(
        requests
            .iter()
            .any(|r| r.contains("login=admin") && r.contains("password=admin")),
        "{requests:?}"
    );
}

#[tokio::test]
async fn only_paths_on_the_local_odoo_are_allowed() {
    let out = tempfile::tempdir().unwrap();
    for page in [
        "https://example.com",
        "//example.com/x",
        "javascript:alert(1)",
        "odoo/sales",
    ] {
        let err = screenshot(
            "http://127.0.0.1:1",
            page,
            Viewport::default(),
            &out.path().join("x.png"),
        )
        .await;
        assert!(err.is_err(), "accepted {page}");
    }
}
