//! Session workspace against a real (temporary) file system.

use std::fs;
use std::thread::sleep;
use std::time::Duration;

use serde_json::Value;
use specs_core::transcript::TranscriptEntry;
use specs_core::workspace::Workspace;
use tempfile::TempDir;

fn workspace() -> (Workspace, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    (Workspace::new(dir.path().join("OdooSpecs")), dir)
}

#[test]
fn new_workspace_is_empty_and_root_is_created() {
    let (ws, _d) = workspace();
    assert!(ws.list().unwrap().is_empty());
    assert!(ws.root().is_dir());
}

#[test]
fn session_folder_layout() {
    let (ws, _d) = workspace();
    ws.create_project("Acme Corp").unwrap();
    let s = ws
        .create_session("Acme Corp", "Delivery window", "claude")
        .unwrap();
    assert_eq!(s.title, "Delivery window");
    assert_eq!(s.provider, "claude");
    assert!(!s.started);

    let dir = ws.session_dir("Acme Corp", &s.id).unwrap();
    assert_eq!(dir, ws.root().join("Acme Corp").join(&s.id));
    assert!(dir.join("screenshots").is_dir());
    let meta: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("session.json")).unwrap()).unwrap();
    assert_eq!(meta["title"], "Delivery window");
    let scene: Value = serde_json::from_str(&ws.read_scene("Acme Corp", &s.id).unwrap()).unwrap();
    assert_eq!(scene["type"], "excalidraw");
    assert_eq!(scene["elements"], Value::Array(vec![]));
}

#[test]
fn invalid_project_names_are_rejected() {
    let (ws, _d) = workspace();
    for bad in ["", "  ", ".", "..", "a/b", ".hidden", "a\\b"] {
        assert!(ws.create_project(bad).is_err(), "accepted {bad:?}");
    }
    assert!(ws.list().unwrap().is_empty());
}

#[test]
fn project_names_are_trimmed_and_unique() {
    let (ws, _d) = workspace();
    ws.create_project("  Acme ").unwrap();
    assert!(ws.create_project("Acme").is_err());
    assert_eq!(ws.list().unwrap()[0].name, "Acme");
}

#[test]
fn projects_by_name_sessions_newest_first() {
    let (ws, _d) = workspace();
    ws.create_project("Zeta").unwrap();
    ws.create_project("Alpha").unwrap();
    let first = ws.create_session("Alpha", "first", "claude").unwrap();
    sleep(Duration::from_millis(5));
    let second = ws.create_session("Alpha", "second", "claude").unwrap();
    sleep(Duration::from_millis(5));
    ws.mark_started("Alpha", &first.id).unwrap();

    let list = ws.list().unwrap();
    assert_eq!(
        list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["Alpha", "Zeta"]
    );
    let ids: Vec<_> = list[0].sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, [first.id.as_str(), second.id.as_str()]);
    assert!(list[0].sessions[0].started);
}

#[test]
fn rename_and_delete_session() {
    let (ws, _d) = workspace();
    ws.create_project("P").unwrap();
    let s = ws.create_session("P", "old", "claude").unwrap();
    assert_eq!(
        ws.rename_session("P", &s.id, "  new ").unwrap().title,
        "new"
    );
    assert_eq!(ws.session("P", &s.id).unwrap().title, "new");
    assert!(ws.rename_session("P", &s.id, " ").is_err());

    let dir = ws.session_dir("P", &s.id).unwrap();
    ws.delete_session("P", &s.id).unwrap();
    assert!(!dir.exists());
    assert!(ws.session("P", &s.id).is_err());
}

#[test]
fn paths_cannot_escape_the_workspace() {
    let (ws, d) = workspace();
    ws.create_project("P").unwrap();
    fs::create_dir_all(d.path().join("outside")).unwrap();
    for id in ["../outside", "..", "/etc", "not-a-uuid"] {
        assert!(ws.session_dir("P", id).is_err(), "accepted id {id:?}");
        assert!(ws.delete_session("P", id).is_err());
    }
    let s = ws.create_session("P", "t", "claude").unwrap();
    assert!(ws.session_dir("../P", &s.id).is_err());
    assert!(d.path().join("outside").exists());
}

#[test]
fn unknown_project_or_session_is_an_error() {
    let (ws, _d) = workspace();
    assert!(ws.create_session("Nope", "t", "claude").is_err());
    ws.create_project("P").unwrap();
    assert!(ws
        .session("P", "6f1c2a7e-0000-4000-8000-000000000000")
        .is_err());
}

#[test]
fn scene_round_trip_and_validation() {
    let (ws, _d) = workspace();
    ws.create_project("P").unwrap();
    let s = ws.create_session("P", "t", "claude").unwrap();
    let scene =
        r#"{"type":"excalidraw","version":2,"elements":[{"id":"x"}],"appState":{},"files":{}}"#;
    ws.write_scene("P", &s.id, scene).unwrap();
    assert_eq!(ws.read_scene("P", &s.id).unwrap(), scene);

    assert!(ws.write_scene("P", &s.id, "not json").is_err());
    assert!(ws.write_scene("P", &s.id, r#"{"type":"other"}"#).is_err());
    assert_eq!(
        ws.read_scene("P", &s.id).unwrap(),
        scene,
        "bad writes must not clobber"
    );
}

#[test]
fn transcript_append_and_read() {
    let (ws, _d) = workspace();
    ws.create_project("P").unwrap();
    let s = ws.create_session("P", "t", "claude").unwrap();
    assert!(ws.read_transcript("P", &s.id).unwrap().is_empty());

    let first = [TranscriptEntry::User { text: "hi".into() }];
    let second = [
        TranscriptEntry::Tool {
            name: "Read".into(),
            is_error: false,
        },
        TranscriptEntry::Assistant {
            text: "hello\nthere".into(),
        },
    ];
    ws.append_transcript("P", &s.id, &first).unwrap();
    ws.append_transcript("P", &s.id, &second).unwrap();

    let path = ws.session_dir("P", &s.id).unwrap().join("transcript.jsonl");
    let mut raw = fs::read_to_string(&path).unwrap();
    raw.push_str("garbage line\n");
    fs::write(&path, raw).unwrap();

    let all: Vec<_> = first.iter().chain(second.iter()).cloned().collect();
    assert_eq!(ws.read_transcript("P", &s.id).unwrap(), all);
}

#[test]
fn conversation_id_is_stored_and_old_sessions_still_load() {
    let (ws, _d) = workspace();
    ws.create_project("P").unwrap();
    let s = ws.create_session("P", "t", "antigravity").unwrap();
    assert_eq!(s.conversation_id, None);
    let saved = ws.set_conversation_id("P", &s.id, "conv-1").unwrap();
    assert_eq!(saved.conversation_id.as_deref(), Some("conv-1"));
    assert_eq!(
        ws.session("P", &s.id).unwrap().conversation_id.as_deref(),
        Some("conv-1")
    );

    // A session.json written before this field existed.
    let path = ws.session_dir("P", &s.id).unwrap().join("session.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("conversation_id");
    std::fs::write(&path, json.to_string()).unwrap();
    assert_eq!(ws.session("P", &s.id).unwrap().conversation_id, None);
}

#[test]
fn a_session_shows_its_database_and_keeps_it_through_other_changes() {
    let (ws, _d) = workspace();
    ws.create_project("Acme").unwrap();
    let s = ws.create_session("Acme", "Spec", "claude").unwrap();
    assert_eq!(s.database, None, "Auto until chosen");

    ws.set_database("Acme", &s.id, Some("specs_db_sales_demo"))
        .unwrap();
    assert_eq!(
        ws.session("Acme", &s.id).unwrap().database.as_deref(),
        Some("specs_db_sales_demo")
    );
    // Other changes to the session do not lose it.
    ws.rename_session("Acme", &s.id, "Renamed").unwrap();
    let listed = &ws.list().unwrap()[0].sessions[0];
    assert_eq!(listed.database.as_deref(), Some("specs_db_sales_demo"));

    ws.set_database("Acme", &s.id, None).unwrap();
    assert_eq!(ws.session("Acme", &s.id).unwrap().database, None);
}

#[test]
fn a_new_session_starts_with_the_clients_latest_hosting() {
    let (ws, _d) = workspace();
    ws.create_project("Acme").unwrap();
    let first = ws.create_session("Acme", "First", "claude").unwrap();
    assert_eq!(first.hosting, None, "nothing to go by yet");
    ws.set_hosting("Acme", &first.id, Some("online")).unwrap();
    sleep(Duration::from_millis(5));
    let second = ws.create_session("Acme", "Second", "claude").unwrap();
    assert_eq!(second.hosting.as_deref(), Some("online"));
    // The client moved to Odoo.sh: from then on, new sessions follow.
    sleep(Duration::from_millis(5));
    ws.set_hosting("Acme", &second.id, Some("sh")).unwrap();
    let third = ws.create_session("Acme", "Third", "claude").unwrap();
    assert_eq!(third.hosting.as_deref(), Some("sh"));
    // Old specs keep theirs.
    assert_eq!(
        ws.session("Acme", &first.id).unwrap().hosting.as_deref(),
        Some("online")
    );
    assert!(ws.set_hosting("Acme", &first.id, Some("cloud")).is_err());
}

#[test]
fn a_session_has_an_odoo_version_inherited_with_the_hosting() {
    let (ws, _d) = workspace();
    ws.create_project("Acme").unwrap();
    let first = ws.create_session("Acme", "First", "claude").unwrap();
    ws.set_hosting("Acme", &first.id, Some("online")).unwrap();
    ws.set_version("Acme", &first.id, Some("saas-19.2"))
        .unwrap();
    sleep(Duration::from_millis(5));
    let next = ws.create_session("Acme", "Next", "claude").unwrap();
    assert_eq!(
        (next.hosting.as_deref(), next.version.as_deref()),
        (Some("online"), Some("saas-19.2"))
    );
    for bad in ["19", "saas-", "../19.0", "19.0; rm"] {
        assert!(
            ws.set_version("Acme", &first.id, Some(bad)).is_err(),
            "{bad}"
        );
    }
}
