//! Asks the real CLIs on this machine whether they are logged in. Skipped by
//! default (depends on the machine). Run with: `make test-live`

#[test]
#[ignore = "depends on the CLIs installed on this machine; run with `make test-live`"]
fn real_provider_status_reports_both_assistants() {
    let statuses = odoo_dev_specs_lib::provider_status();
    for s in &statuses {
        eprintln!(
            "{}: installed={} logged_in={}",
            s.name, s.installed, s.logged_in
        );
    }
    assert_eq!(statuses.len(), 2);
    // On the developer machine both are installed and logged in.
    assert!(statuses.iter().all(|s| s.ready()), "{statuses:?}");
}
