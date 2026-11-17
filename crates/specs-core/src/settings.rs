//! User settings, stored as one small JSON file. Unknown or missing fields fall
//! back to defaults, so an older or hand-edited file never breaks the app.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::fsutil::write_atomic;

/// Which AI assistant answers by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    Claude,
}

/// Persisted preferences. `#[serde(default)]` keeps every field optional on disk,
/// so a missing field falls back to its default instead of failing to load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// AI assistant used for new sessions.
    pub provider: Provider,
    /// Odoo version used for screenshots when the user does not name one.
    pub default_version: String,
    /// Create screenshot databases with Odoo demo data (realistic records).
    pub demo_data: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: Provider::Claude,
            default_version: "19.0".into(),
            demo_data: true,
        }
    }
}

/// The settings file: `<config>/odoo-dev-specs/settings.json`.
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_default()
            .join("odoo-dev-specs")
            .join("settings.json")
    }

    /// Load the settings, or the defaults when the file is missing or unreadable.
    pub fn load(&self) -> Settings {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &Settings) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        write_atomic(
            &self.path,
            serde_json::to_string_pretty(settings)?.as_bytes(),
        )
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
