//! Real headless Chrome against a tiny fake Odoo served by the test.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use specs_mcp::browser::{screenshot, screenshots, Shot, Viewport};

const LOGIN: &str = r#"<html><body><form action="/after" method="get">
<input name="login"><input name="password" type="password"><button type="submit">Log in</button>
</form></body></html>"#;

/// Like Odoo, the view appears a moment after the page loads.
const AFTER: &str = r#"<html><body style="background:#714b67"><div class="o_action_manager"></div>
<script>setTimeout(() => {
  document.querySelector('.o_action_manager').innerHTML = '<div class="o_view_controller">Sales</div>';
}, 300);</script></body></html>"#;

/// An Odoo-like form and list at known positions (absolute, so they are exact).
const FORM: &str = r#"<html><body style="margin:0"><div class="o_action_manager"><div class="o_view_controller">
<label class="o_form_label" for="validity_date_0" style="position:absolute;left:470px;top:240px;width:80px;height:20px">Expiration</label>
<div class="o_field_widget" name="validity_date" style="position:absolute;left:590px;top:236px;width:295px;height:24px">Dec 17</div>
<button name="action_confirm" style="position:absolute;left:147px;top:116px;width:82px;height:36px">Confirm</button>
<table style="position:absolute;left:20px;top:400px"><tr><th data-name="product_id" style="width:300px;height:30px">Product</th></tr></table>
<div class="o_field_widget" name="hidden_one" style="display:none">x</div>
</div></div></body></html>"#;

/// A report as /report/html serves it: no Odoo web client around it, and no fields,
/// so its visible texts are what can be pointed at.
const REPORT: &str = r#"<html><body style="margin:0"><main><div class="article"><div class="page">
<h2 style="position:absolute;left:60px;top:500px;margin:0;font-size:20px">Quotation # S00003</h2>
<span style="position:absolute;left:470px;top:440px">Ready Mat</span>
</div></div></main></body></html>"#;

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
            let body = if request.contains(" /after") || request.contains(" /odoo/sales") {
                AFTER
            } else if request.contains(" /form") {
                FORM
            } else if request.contains(" /report") {
                REPORT
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

#[tokio::test]
async fn a_batch_uses_one_chrome_logs_in_once_and_maps_what_is_on_screen() {
    let (url, seen) = fake_odoo();
    let out = tempfile::tempdir().unwrap();
    let shot = |page: &str, name: &str| Shot {
        page: page.into(),
        viewport: Viewport {
            width: 1000,
            height: 700,
        },
        out: out.path().join(name),
    };
    let started = std::time::Instant::now();
    let maps = tokio::time::timeout(
        Duration::from_secs(90),
        screenshots(
            &url,
            &[
                shot("/form", "form.png"),
                shot("/report/html/x", "report.png"),
            ],
        ),
    )
    .await
    .expect("timed out")
    .unwrap();
    // A report page is taken as soon as it is loaded, not after the 45 s wait for
    // an Odoo view that never comes.
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );

    for name in ["form.png", "report.png"] {
        assert_eq!(
            png_size(&std::fs::read(out.path().join(name)).unwrap()),
            (1000, 700)
        );
    }
    let requests = seen.lock().unwrap().clone();
    let logins = requests
        .iter()
        .filter(|r| r.contains("login=admin"))
        .count();
    assert_eq!(logins, 1, "logged in once for the batch: {requests:?}");

    let form = &maps[0];
    let find = |kind: &str, name: &str| {
        form.iter()
            .find(|e| e.kind == kind && e.name == name)
            .unwrap_or_else(|| panic!("no {kind} {name} in {form:?}"))
    };
    let field = find("field", "validity_date");
    assert_eq!(
        (field.x, field.y, field.w, field.h),
        (590.0, 236.0, 295.0, 24.0)
    );
    assert_eq!(field.text, "Dec 17");
    let label = find("label", "validity_date");
    assert_eq!((label.x, label.y), (470.0, 240.0));
    assert_eq!(label.text, "Expiration");
    assert_eq!(find("button", "action_confirm").text, "Confirm");
    assert_eq!(find("column", "product_id").text, "Product");
    assert!(
        form.iter().all(|e| e.name != "hidden_one"),
        "hidden elements are left out"
    );

    // A report has no fields: its visible texts are mapped instead.
    let report = &maps[1];
    let heading = report
        .iter()
        .find(|e| e.text == "Quotation # S00003")
        .unwrap_or_else(|| panic!("{report:?}"));
    assert_eq!(
        (heading.kind.as_str(), heading.x, heading.y),
        ("text", 60.0, 500.0)
    );
}
