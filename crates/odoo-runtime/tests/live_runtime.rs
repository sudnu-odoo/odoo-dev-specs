//! Against the real git, Python, PostgreSQL and Odoo source: downloads Odoo 19.0
//! into the app's runtime folder (reused by the app afterwards), creates a fresh
//! database and serves it. Slow the first time (download + Python packages), so
//! skipped by default. Run with `make test-live`.

use std::io::{Read, Write};
use std::net::TcpStream;

use odoo_runtime::runtime::{Edition, Runtime, SourceState, Tools};

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

/// A real download from GitHub with the app's key: listed quickly, saved in a folder
/// named by its version, shown with that folder, and freed again on removal.
#[test]
#[ignore = "downloads Odoo from GitHub; run with `make test-live`"]
fn real_download_goes_to_a_folder_named_by_its_version() {
    let downloads = tempfile::tempdir().unwrap();
    let rt = Runtime::for_app(Runtime::default_root(), Tools::default())
        .with_downloads_dir(downloads.path());
    let started = std::time::Instant::now();
    let published = rt.available_versions().unwrap();
    eprintln!("published versions in {:?}", started.elapsed());
    assert!(published.contains(&"17.0".to_string()), "{published:?}");
    if rt.source_state("17.0", Edition::Community) != SourceState::Missing {
        eprintln!("17.0 is already on this computer: nothing to download");
        return;
    }

    let started = std::time::Instant::now();
    let mut last = 0;
    rt.download_source("17.0", Edition::Community, &mut |p| last = p)
        .unwrap();
    eprintln!("downloaded 17.0 Community in {:?}", started.elapsed());
    assert_eq!(last, 100);
    let folder = downloads.path().join("17.0/odoo");
    assert!(folder.join("odoo-bin").is_file());
    let info = rt
        .downloaded_versions()
        .into_iter()
        .find(|v| v.version == "17.0")
        .unwrap();
    assert_eq!(info.community_path.as_deref(), Some(folder.as_path()));
    assert_eq!(info.community, SourceState::Ready { linked: false });

    rt.remove_version("17.0").unwrap();
    assert!(!folder.exists());
    assert_eq!(
        rt.source_state("17.0", Edition::Community),
        SourceState::Missing
    );
}
