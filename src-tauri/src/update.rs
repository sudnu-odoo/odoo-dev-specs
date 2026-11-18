//! Updates from the project's GitHub Releases (like Atoll): is there a newer
//! version, what changed, and installing its .deb with Ubuntu's password window,
//! then restarting into it. Development builds never offer updates.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

/// The version of this build: CI stamps the release version in (`SPECS_VERSION`),
/// local builds use Cargo.toml's.
pub const VERSION: &str = match option_env!("SPECS_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

const TIMEOUT: Duration = Duration::from_secs(20);

/// A newer version, as the update banner shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// "0.2.3", without the "v".
    pub version: String,
    /// What changed, as written in the release.
    pub notes: String,
    pub deb_url: String,
}

/// "owner/name", from Cargo.toml's `repository`.
fn repo() -> &'static str {
    env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/")
}

#[derive(Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// Whether `a` is a later version than `b` ("0.1.10" > "0.1.9").
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| {
                p.trim_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    };
    parts(a) > parts(b)
}

/// The update a GitHub release offers to a build at `current`, if any.
pub fn update_from(release: GhRelease, current: &str) -> Option<Release> {
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !newer(&version, current) {
        return None;
    }
    let deb = release
        .assets
        .into_iter()
        .find(|a| a.name.ends_with("_amd64.deb"))?;
    Some(Release {
        version,
        notes: release.body.unwrap_or_default().trim().to_string(),
        deb_url: deb.browser_download_url,
    })
}

fn client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("OdooDevSpecs/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// A newer version on GitHub, if there is one. Development builds say no.
#[tauri::command]
pub async fn check_update() -> Result<Option<Release>, String> {
    if cfg!(debug_assertions) {
        return Ok(None);
    }
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo());
    let release: GhRelease = client()
        .map(|c| c.get(&url))
        .map_err(|e| e.to_string())?
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Could not check for updates: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Could not read the update information: {e}"))?;
    Ok(update_from(release, VERSION))
}

/// Download the release's .deb, install it with the password window, restart.
#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>, release: Release) -> Result<(), String> {
    let dir = cache_dir().ok_or("There is no cache folder to download the update to.")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("odoo-dev-specs_{}_amd64.deb", release.version));
    let bytes = client()
        .map_err(|e| e.to_string())?
        .get(&release.deb_url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("The download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("The download failed: {e}"))?;
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let installed = tauri::async_runtime::spawn_blocking({
        let path = path.clone();
        move || {
            std::process::Command::new("pkexec")
                .args(["apt-get", "install", "-y", "--allow-downgrades"])
                .arg(&path)
                .status()
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|_| "The password window (pkexec) is not available.".to_string())?;
    let _ = std::fs::remove_file(&path);
    if !installed.success() {
        return Err(
            "The update was not installed (the password was not given, or it failed).".into(),
        );
    }
    app.restart();
}

fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|c| c.join("odoo-dev-specs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, assets: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: tag.into(),
            body: Some("- Pick the database per session\n".into()),
            assets: assets
                .iter()
                .map(|n| GhAsset {
                    name: n.to_string(),
                    browser_download_url: format!("https://example.invalid/{n}"),
                })
                .collect(),
        }
    }

    #[test]
    fn compares_versions_as_numbers() {
        assert!(newer("0.1.10", "0.1.9"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(!newer("0.1.9", "0.1.9"));
        assert!(!newer("0.1.8", "0.1.9"));
    }

    #[test]
    fn a_newer_release_with_a_deb_is_offered_with_its_notes() {
        let update = update_from(
            release("v0.2.0", &["odoo-dev-specs_0.2.0_amd64.deb", "source.zip"]),
            "0.1.4",
        )
        .unwrap();
        assert_eq!(update.version, "0.2.0");
        assert_eq!(update.notes, "- Pick the database per session");
        assert!(update.deb_url.ends_with("odoo-dev-specs_0.2.0_amd64.deb"));
    }

    #[test]
    fn nothing_is_offered_when_up_to_date_or_without_a_package() {
        assert_eq!(
            update_from(release("v0.1.4", &["x_0.1.4_amd64.deb"]), "0.1.4"),
            None
        );
        assert_eq!(
            update_from(release("v0.2.0", &["notes.txt"]), "0.1.4"),
            None
        );
    }

    #[test]
    fn the_repository_is_the_public_one() {
        assert_eq!(repo(), "sudnu-odoo/odoo-dev-specs");
    }
}
