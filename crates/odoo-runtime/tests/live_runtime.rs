//! Against the real git, Python, PostgreSQL and Odoo source: downloads Odoo 19.0
//! into the app's runtime folder (reused by the app afterwards), creates a fresh
//! database and serves it. Slow the first time (download + Python packages), so
//! skipped by default. Run with `make test-live`.

use std::io::{Read, Write};
use std::net::TcpStream;

use odoo_runtime::runtime::{Runtime, Tools};

#[test]
#[ignore = "downloads and runs a real Odoo; run with `make test-live`"]
fn real_runtime_sets_up_creates_and_serves_a_fresh_database() {
    let rt = Runtime::new(Runtime::default_root(), Tools::default());
    let db = rt
        .ensure_database("19.0", &["contacts".into()], true)
        .unwrap();
    assert_eq!(db, "specs_19_0_contacts");
    assert!(rt.databases().unwrap().contains(&db));
    assert!(rt.source_dirs().contains(&rt.source_dir("19.0")));

    let running = rt.ensure_running("19.0", &db).unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", running.port)).unwrap();
    write!(stream, "GET /web/login HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    rt.stop_all().unwrap();
    assert!(
        response.starts_with("HTTP/1.0 200") || response.starts_with("HTTP/1.1 200"),
        "{}",
        &response[..300.min(response.len())]
    );
    assert!(response.contains("name=\"login\""), "login form is served");
}
