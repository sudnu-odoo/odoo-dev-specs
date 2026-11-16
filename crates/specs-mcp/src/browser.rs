//! Screenshots of a local Odoo with headless Chrome, logged in as admin.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::handler::viewport::Viewport as ChromeViewport;
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;

/// Longest wait for Odoo to finish drawing a view; after that the page is taken as is.
const RENDER_TIMEOUT: Duration = Duration::from_secs(45);
/// Extra time for animations and late images once Odoo looks ready.
const SETTLE: Duration = Duration::from_millis(600);
/// Submits Odoo's login form as admin when the page shows one; returns whether it did.
const LOGIN_JS: &str = "(() => { \
    const login = document.querySelector('input[name=login]'); \
    if (!login || !login.form) return false; \
    login.value = 'admin'; \
    login.form.querySelector('input[name=password]').value = 'admin'; \
    login.form.submit(); \
    return true; })()";
/// True once an Odoo view is drawn and nothing is loading any more.
const READY_JS: &str = "!!document.querySelector('.o_action_manager > *, .o_home_menu') \
    && !document.querySelector('.o_loading_indicator, .o_blockUI, .o_loading')";

/// Size of the browser window, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1440,
            height: 900,
        }
    }
}

/// Open `page` (a path like `/odoo/sales`) on the Odoo at `base_url`, log in as
/// admin/admin if asked to, wait for the view to render and save a PNG to `out`.
pub async fn screenshot(base_url: &str, page: &str, viewport: Viewport, out: &Path) -> Result<()> {
    ensure!(
        page.starts_with('/') && !page.starts_with("//"),
        "Give a path on the local Odoo starting with /, like /odoo/sales."
    );
    let profile = tempfile::tempdir().context("creating a browser profile")?;
    let config = BrowserConfig::builder()
        .new_headless_mode()
        .window_size(viewport.width, viewport.height)
        .viewport(ChromeViewport {
            width: viewport.width,
            height: viewport.height,
            ..ChromeViewport::default()
        })
        .user_data_dir(profile.path())
        .arg("--hide-scrollbars")
        .build()
        .map_err(|e| anyhow!("Chrome is not available: {e}"))?;
    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .context("Could not start Chrome. Is Google Chrome or Chromium installed?")?;
    let events = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = capture(&browser, base_url, page, out).await;
    let _ = browser.close().await;
    let _ = browser.wait().await;
    events.abort();
    result
}

async fn capture(browser: &Browser, base_url: &str, page: &str, out: &Path) -> Result<()> {
    let url = format!("{base_url}/web/login?redirect={}", percent_encode(page));
    let tab = browser
        .new_page(url.as_str())
        .await
        .context("Could not open Odoo in Chrome.")?;
    tab.wait_for_navigation().await?;

    // Fill and submit the login form directly: clicking fields depends on layout and
    // fails on Odoo's real login page. The form carries its own CSRF token.
    let submitted = tab
        .evaluate(LOGIN_JS)
        .await?
        .into_value::<bool>()
        .unwrap_or(false);
    if submitted {
        tab.wait_for_navigation().await?;
    }

    let started = Instant::now();
    while started.elapsed() < RENDER_TIMEOUT {
        let ready = tab
            .evaluate(READY_JS)
            .await
            .ok()
            .and_then(|r| r.into_value::<bool>().ok());
        if ready == Some(true) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    tokio::time::sleep(SETTLE).await;

    let png = tab
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .build(),
        )
        .await
        .context("Chrome could not take the screenshot.")?;
    std::fs::write(out, png).with_context(|| format!("Could not save {}.", out.display()))
}

/// Encode everything except unreserved URL characters.
fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
