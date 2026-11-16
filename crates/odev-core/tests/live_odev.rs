//! Against the real odev, PostgreSQL and Odoo source: creates a fresh database
//! and starts it. Slow (minutes), so skipped by default. Run with `make test-live`.

use std::io::{Read, Write};
use std::net::TcpStream;

use odev_core::server::Odev;

fn odev() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap();
    [
        home.join(".local/bin/odev"),
        "/usr/local/bin/odev".into(),
        "/usr/bin/odev".into(),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .expect("odev is not installed")
}

#[test]
#[ignore = "creates and runs a real Odoo database; run with `make test-live`"]
fn real_odev_creates_and_serves_a_fresh_database() {
    let state = tempfile::tempdir().unwrap();
    let odev = Odev::new(odev(), state.path());
    let db = odev
        .ensure_database("19.0", &["sale_management".into()], true)
        .unwrap();
    assert!(odev.databases().unwrap().contains(&db));

    let running = odev.ensure_running("19.0", &db).unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", running.port)).unwrap();
    write!(stream, "GET /web/login HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    odev.stop_all().unwrap();
    assert!(
        response.starts_with("HTTP/1.0 200") || response.starts_with("HTTP/1.1 200"),
        "{}",
        &response[..200.min(response.len())]
    );
    assert!(response.contains("name=\"login\""), "login form is served");
}
