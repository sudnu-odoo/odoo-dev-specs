//! Settings persistence against a temporary file.

use std::fs;

use specs_core::settings::{Provider, Settings, SettingsStore};
use tempfile::TempDir;

fn store() -> (SettingsStore, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    (
        SettingsStore::new(dir.path().join("cfg").join("settings.json")),
        dir,
    )
}

#[test]
fn defaults_before_anything_is_saved() {
    let (store, _d) = store();
    let s = store.load();
    assert_eq!(s, Settings::default());
    assert_eq!(s.provider, Provider::Claude);
    assert_eq!(s.default_version, "19.0");
    assert!(s.demo_data);
}

#[test]
fn saved_settings_round_trip_and_create_the_folder() {
    let (store, _d) = store();
    let settings = Settings {
        provider: Provider::Gemini,
        default_version: "18.0".into(),
        demo_data: false,
    };
    store.save(&settings).unwrap();
    assert!(store.path().is_file());
    assert_eq!(store.load(), settings);
}

#[test]
fn unknown_fields_are_ignored_and_missing_ones_default() {
    let (store, _d) = store();
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    // Only one known field set, plus a field the app does not know.
    fs::write(
        store.path(),
        r#"{"defaultVersion":"17.0","future":"value"}"#,
    )
    .unwrap();
    let s = store.load();
    assert_eq!(s.default_version, "17.0");
    assert_eq!(s.provider, Provider::Claude);
    assert!(s.demo_data);
}

#[test]
fn a_corrupt_file_falls_back_to_defaults() {
    let (store, _d) = store();
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    fs::write(store.path(), "not json").unwrap();
    assert_eq!(store.load(), Settings::default());
}

#[test]
fn provider_serialises_in_snake_case() {
    assert_eq!(
        serde_json::to_string(&Provider::Claude).unwrap(),
        r#""claude""#
    );
    assert_eq!(
        serde_json::to_string(&Provider::Gemini).unwrap(),
        r#""gemini""#
    );
}
