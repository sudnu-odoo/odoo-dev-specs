//! Screenshots of a local Odoo with headless Chrome, logged in as admin.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::handler::viewport::Viewport as ChromeViewport;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

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
/// Visible fields, labels, buttons, list columns and tabs with their boxes; on pages
/// without any (reports), short visible texts instead. Returned as a JSON string.
const ELEMENTS_JS: &str = r#"(() => {
  const out = [], seen = new Set();
  const add = (kind, name, text, el) => {
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2 || r.bottom <= 0 || r.right <= 0 ||
        r.top >= innerHeight || r.left >= innerWidth) return;
    const st = getComputedStyle(el);
    if (st.visibility === 'hidden' || st.display === 'none' || st.opacity === '0') return;
    const t = (text || '').replace(/\s+/g, ' ').trim().slice(0, 60);
    const key = [kind, name, t, Math.round(r.x), Math.round(r.y)].join('|');
    if (seen.has(key)) return;
    seen.add(key);
    out.push({ kind, name: name || '', text: t, x: Math.round(r.x), y: Math.round(r.y),
               w: Math.round(r.width), h: Math.round(r.height) });
  };
  const all = (sel, f) => document.querySelectorAll(sel).forEach(f);
  all('.o_field_widget[name]', el => add('field', el.getAttribute('name'), el.innerText, el));
  all('.o_form_label, label[for]', el =>
    add('label', (el.getAttribute('for') || '').replace(/_\d+$/, ''), el.innerText, el));
  all('th[data-name]', el => add('column', el.dataset.name, el.innerText, el));
  all('button, a.btn, .oe_stat_button', el => add('button', el.getAttribute('name'), el.innerText, el));
  all('.o_notebook_headers .nav-link, .nav-tabs .nav-link', el =>
    add('tab', el.getAttribute('name'), el.innerText, el));
  if (!out.some(e => e.kind === 'field' || e.kind === 'column')) {
    all('h1, h2, h3, h4, strong, span, td, th, p, div', el => {
      if (el.children.length) return;
      const t = (el.innerText || '').trim();
      if (t && t.length <= 60) add('text', '', t, el);
    });
  }
  return JSON.stringify(out.slice(0, 300));
})()"#;

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

/// One page to capture: where to go, how large, and where to save the PNG.
#[derive(Debug, Clone)]
pub struct Shot {
    /// A path on the local Odoo, like `/odoo/sales`.
    pub page: String,
    pub viewport: Viewport,
    pub out: PathBuf,
}

/// Something visible on a screenshot, in the screenshot's own pixels: a field, its
/// label, a button, a list column, a tab, or (on pages without fields, such as
/// reports) a short visible text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub kind: String,
    /// Technical name when Odoo gives one (field, column, button), else empty.
    pub name: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Open `page` (a path like `/odoo/sales`) on the Odoo at `base_url`, log in as
/// admin/admin if asked to, wait for the view to render and save a PNG to `out`.
pub async fn screenshot(base_url: &str, page: &str, viewport: Viewport, out: &Path) -> Result<()> {
    screenshots(
        base_url,
        &[Shot {
            page: page.into(),
            viewport,
            out: out.into(),
        }],
    )
    .await
    .map(|_| ())
}

/// Capture several pages with one Chrome, logged in once, and map what is on each:
/// returns, per shot, the positions of its fields, labels, buttons and so on.
pub async fn screenshots(base_url: &str, shots: &[Shot]) -> Result<Vec<Vec<Element>>> {
    for shot in shots {
        ensure!(
            shot.page.starts_with('/') && !shot.page.starts_with("//"),
            "Give a path on the local Odoo starting with /, like /odoo/sales."
        );
    }
    let Some(first) = shots.first() else {
        return Ok(vec![]);
    };
    let profile = tempfile::tempdir().context("creating a browser profile")?;
    let config = BrowserConfig::builder()
        .new_headless_mode()
        .window_size(first.viewport.width, first.viewport.height)
        .viewport(ChromeViewport {
            width: first.viewport.width,
            height: first.viewport.height,
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

    let result = capture_all(&browser, base_url, shots).await;
    let _ = browser.close().await;
    let _ = browser.wait().await;
    events.abort();
    result
}

async fn capture_all(
    browser: &Browser,
    base_url: &str,
    shots: &[Shot],
) -> Result<Vec<Vec<Element>>> {
    // Log in through the login page once; the session cookie serves every page after.
    let login_url = format!(
        "{base_url}/web/login?redirect={}",
        percent_encode(&shots[0].page)
    );
    let tab = browser
        .new_page(login_url.as_str())
        .await
        .context("Could not open Odoo in Chrome.")?;
    tab.wait_for_navigation().await?;
    log_in_if_asked(&tab).await?;

    let mut maps = Vec::with_capacity(shots.len());
    for shot in shots {
        tab.execute(SetDeviceMetricsOverrideParams::new(
            shot.viewport.width,
            shot.viewport.height,
            1.0,
            false,
        ))
        .await
        .context("Could not size the browser window.")?;
        tab.goto(format!("{base_url}{}", shot.page))
            .await
            .with_context(|| format!("Could not open {} in Chrome.", shot.page))?;
        tab.wait_for_navigation().await?;
        // The session may have expired: then this page is the login form.
        log_in_if_asked(&tab).await?;
        wait_until_drawn(&tab).await;

        let png = tab
            .screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build(),
            )
            .await
            .context("Chrome could not take the screenshot.")?;
        std::fs::write(&shot.out, png)
            .with_context(|| format!("Could not save {}.", shot.out.display()))?;
        let found: String = tab
            .evaluate(ELEMENTS_JS)
            .await
            .ok()
            .and_then(|r| r.into_value().ok())
            .unwrap_or_default();
        maps.push(serde_json::from_str(&found).unwrap_or_default());
    }
    Ok(maps)
}

async fn log_in_if_asked(tab: &Page) -> Result<()> {
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
    Ok(())
}

async fn wait_until_drawn(tab: &Page) {
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
