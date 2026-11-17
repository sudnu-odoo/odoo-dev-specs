//! MCP server giving the assistant tools to edit one session's diagram
//! (`<session>/spec.excalidraw`). Claude Code starts it per chat turn.

pub mod browser;
mod options;

pub use options::{parse_args, Options};

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use browser::Viewport;
use excalidraw::{Color, Placement, Scene, Target};
use odoo_runtime::runtime::Runtime;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

pub const SCENE_FILE: &str = "spec.excalidraw";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddBox {
    /// Text shown in the box. Use \n for line breaks.
    pub label: String,
    /// Left position. Leave out x and y to place the box below the existing content.
    pub x: Option<f64>,
    /// Top position.
    pub y: Option<f64>,
    /// Width in pixels. Leave out to fit the label.
    pub width: Option<f64>,
    /// Height in pixels. Leave out to fit the label.
    pub height: Option<f64>,
    /// One of: gray, blue, green, yellow, red, purple.
    pub color: Option<String>,
    /// Id of the frame (screen) this box belongs to.
    pub frame_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddText {
    pub text: String,
    /// Screenshot to write on (e.g. a new field's label where it will appear). With it,
    /// x and y are pixels of that PNG file; without it, canvas coordinates.
    pub image_id: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Default 20. Use 28-36 for titles.
    pub font_size: Option<f64>,
    /// One of: gray, blue, green, yellow, red, purple. Use red for annotations
    /// (what is new or changes); leave out for normal text.
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddPointer {
    /// Id of the note (usually a red text) the arrow starts from.
    pub from_id: String,
    /// Screenshot the arrow points into. With it, x and y are pixels of that PNG file
    /// (as you see the image when you open it); without it, canvas coordinates.
    pub image_id: Option<String>,
    pub x: f64,
    pub y: f64,
    /// Default red.
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddHighlight {
    /// Screenshot the outline is drawn on. With it, x, y, width and height are pixels
    /// of that PNG file; without it, canvas coordinates.
    pub image_id: Option<String>,
    /// Left of the outlined area.
    pub x: f64,
    /// Top of the outlined area.
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Default red.
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddArrow {
    /// Id of the element the arrow starts from.
    pub from_id: String,
    /// Id of the element the arrow points to.
    pub to_id: String,
    /// Short text on the arrow, e.g. "creates".
    pub label: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddFrame {
    /// Name shown above the frame, e.g. "Sales order form".
    pub name: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddImage {
    /// PNG file inside the session folder, e.g. "screenshots/sale_order.png".
    pub path: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Display width; the height follows the image proportions.
    pub width: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetText {
    /// Id of a text, box, arrow or frame.
    pub id: String,
    pub text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveElement {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteElements {
    pub ids: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Screenshot {
    /// Odoo version, e.g. "19.0".
    pub version: String,
    /// Technical names of the modules to install, e.g. ["sale_management", "stock"].
    pub modules: Vec<String>,
    /// Page to open, as a path on the local Odoo, e.g. "/odoo/sales" or "/odoo/action-sale.action_orders".
    pub page: String,
    /// Short file name for the screenshot, e.g. "sales_order_form".
    pub name: String,
    /// Browser width in pixels (default 1440).
    pub width: Option<u32>,
    /// Browser height in pixels (default 900).
    pub height: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct DiagramServer {
    session_dir: PathBuf,
    /// One edit at a time: each tool reads, changes and writes the whole file.
    lock: Arc<tokio::sync::Mutex<()>>,
    /// Local Odoo for screenshots; `None` when screenshots are not enabled.
    runtime: Option<Arc<Runtime>>,
    /// Create screenshot databases with Odoo demo data (realistic records).
    demo_data: bool,
}

impl DiagramServer {
    pub fn new(session_dir: impl Into<PathBuf>) -> Self {
        Self {
            session_dir: session_dir.into(),
            lock: Arc::default(),
            runtime: None,
            demo_data: true,
        }
    }

    /// Enable Odoo screenshots through this runtime.
    pub fn with_runtime(mut self, runtime: Runtime) -> Self {
        self.runtime = Some(Arc::new(runtime));
        self
    }

    /// Whether screenshot databases load Odoo demo data (default true).
    pub fn with_demo_data(mut self, demo_data: bool) -> Self {
        self.demo_data = demo_data;
        self
    }
}

#[tool_router]
impl DiagramServer {
    #[tool(
        description = "List the elements of the diagram (ids, kinds, texts, positions, sizes). Call this before changing existing elements."
    )]
    async fn diagram_summary(&self) -> CallToolResult {
        let _guard = self.lock.lock().await;
        reply(
            self.load()
                .and_then(|scene| Ok(serde_json::to_string_pretty(&scene.summary())?)),
        )
    }

    #[tool(
        description = "Add a box with a label, e.g. a model, a screen part or a step. Returns its id."
    )]
    async fn diagram_add_box(&self, Parameters(p): Parameters<AddBox>) -> CallToolResult {
        self.edit(|scene| {
            let color = p.color.as_deref().map(parse_color).transpose()?;
            let size = match (p.width, p.height) {
                (Some(w), Some(h)) => Some((w, h)),
                (None, None) => None,
                _ => bail!("Give both width and height, or neither."),
            };
            let id = scene.add_box(
                &unescape(&p.label),
                Placement { x: p.x, y: p.y },
                size,
                color,
                p.frame_id.as_deref(),
            )?;
            let (w, h) = scene.size_of(&id).unwrap_or_default();
            // The box may be taller than asked (to fit its label): say so for the layout.
            Ok(format!(
                "Added box {id} ({w}x{h}).{}",
                overlap_note(scene, &id)
            ))
        })
        .await
    }

    #[tool(
        description = "Add free text such as a title, a short explanation or a red annotation note. Returns its id."
    )]
    async fn diagram_add_text(&self, Parameters(p): Parameters<AddText>) -> CallToolResult {
        self.edit(|scene| {
            let color = p.color.as_deref().map(parse_color).transpose()?;
            let at = match (&p.image_id, p.x, p.y) {
                (Some(image), Some(x), Some(y)) => {
                    let (x, y) = scene.resolve(&target(Some(image.clone()), x, y))?;
                    Placement {
                        x: Some(x),
                        y: Some(y),
                    }
                }
                (Some(_), _, _) => bail!("Give x and y with image_id."),
                (None, x, y) => Placement { x, y },
            };
            let id = scene.add_text(&unescape(&p.text), at, p.font_size, color);
            // It may have been moved down so as not to cover another text: say where.
            let at = scene
                .summary()
                .into_iter()
                .find(|e| e.id == id)
                .map(|e| format!(" at ({}, {})", e.x, e.y))
                .unwrap_or_default();
            Ok(format!("Added text {id}{at}.{}", overlap_note(scene, &id)))
        })
        .await
    }

    #[tool(
        description = "Draw an annotation arrow from a note to a spot, e.g. the field on a screenshot the note talks about. Give image_id to use the screenshot's own pixel coordinates. Returns its id."
    )]
    async fn diagram_add_pointer(&self, Parameters(p): Parameters<AddPointer>) -> CallToolResult {
        self.edit(|scene| {
            let color = p.color.as_deref().map(parse_color).transpose()?;
            let target = target(p.image_id, p.x, p.y);
            let id = scene.add_pointer(&p.from_id, &target, color)?;
            Ok(format!("Added pointer {id}."))
        })
        .await
    }

    #[tool(
        description = "Outline (no fill) part of a screenshot or of the canvas, to mark what is new, hidden or changed. Give image_id to use the screenshot's own pixel coordinates. Returns its id."
    )]
    async fn diagram_add_highlight(
        &self,
        Parameters(p): Parameters<AddHighlight>,
    ) -> CallToolResult {
        self.edit(|scene| {
            let color = p.color.as_deref().map(parse_color).transpose()?;
            let target = target(p.image_id, p.x, p.y);
            let id = scene.add_highlight(&target, (p.width, p.height), color)?;
            Ok(format!("Added highlight {id}."))
        })
        .await
    }

    #[tool(
        description = "Connect two elements with an arrow, optionally labelled. Returns its id."
    )]
    async fn diagram_add_arrow(&self, Parameters(p): Parameters<AddArrow>) -> CallToolResult {
        self.edit(|scene| {
            let label = p.label.as_deref().map(unescape);
            let id = scene.add_arrow(&p.from_id, &p.to_id, label.as_deref())?;
            Ok(format!("Added arrow {id}."))
        })
        .await
    }

    #[tool(
        description = "Add a named frame grouping one screen or one part of the spec. Returns its id."
    )]
    async fn diagram_add_frame(&self, Parameters(p): Parameters<AddFrame>) -> CallToolResult {
        self.edit(|scene| {
            let id = scene.add_frame(
                &unescape(&p.name),
                Placement { x: p.x, y: p.y },
                (p.width, p.height),
            );
            Ok(format!("Added frame {id}."))
        })
        .await
    }

    #[tool(
        description = "Place a PNG from the session folder (e.g. a screenshot) on the diagram. Returns its id."
    )]
    async fn diagram_add_image(&self, Parameters(p): Parameters<AddImage>) -> CallToolResult {
        let png = match self.session_file(&p.path) {
            Ok(path) => fs::read(&path).with_context(|| format!("Could not read {}.", p.path)),
            Err(e) => Err(e),
        };
        self.edit(|scene| {
            let id = scene.add_image(&png?, Placement { x: p.x, y: p.y }, p.width)?;
            Ok(format!("Added image {id}."))
        })
        .await
    }

    #[tool(description = "Change the text of a text, box label, arrow label or frame name.")]
    async fn diagram_set_text(&self, Parameters(p): Parameters<SetText>) -> CallToolResult {
        self.edit(|scene| {
            scene.set_text(&p.id, &unescape(&p.text))?;
            Ok(format!(
                "Changed the text of {}.{}",
                p.id,
                overlap_note(scene, &p.id)
            ))
        })
        .await
    }

    #[tool(description = "Move an element (its label, frame content and arrows follow).")]
    async fn diagram_move(&self, Parameters(p): Parameters<MoveElement>) -> CallToolResult {
        self.edit(|scene| {
            scene.move_to(&p.id, p.x, p.y)?;
            Ok(format!("Moved {}.{}", p.id, overlap_note(scene, &p.id)))
        })
        .await
    }

    #[tool(
        description = "Take a screenshot of a page of a fresh local Odoo database (no customer data), \
logged in as admin, and save it in the session's screenshots folder. The first call for a version \
and module set creates the database, which can take several minutes. Then place the image with \
diagram_add_image."
    )]
    async fn odoo_screenshot(&self, Parameters(p): Parameters<Screenshot>) -> CallToolResult {
        reply(self.take_screenshot(p).await)
    }

    #[tool(description = "Delete elements, with their labels and attached arrows.")]
    async fn diagram_delete(&self, Parameters(p): Parameters<DeleteElements>) -> CallToolResult {
        self.edit(|scene| {
            let n = scene.delete(&p.ids)?;
            Ok(format!("Deleted {n} elements."))
        })
        .await
    }
}

impl DiagramServer {
    async fn take_screenshot(&self, p: Screenshot) -> Result<String> {
        let file = format!("{}.png", file_stem(&p.name)?);
        let runtime = self
            .runtime
            .clone()
            .context("Odoo screenshots are not available: the local Odoo runtime is not set up.")?;
        let (version, modules, demo_data) = (p.version.clone(), p.modules.clone(), self.demo_data);
        let running = tokio::task::spawn_blocking(move || {
            let database = runtime.ensure_database(&version, &modules, demo_data)?;
            runtime.ensure_running(&version, &database)
        })
        .await
        .context("starting Odoo")??;

        let relative = format!("screenshots/{file}");
        let out = self.session_dir.join(&relative);
        fs::create_dir_all(self.session_dir.join("screenshots"))?;
        let viewport = Viewport {
            width: p.width.unwrap_or(1440).clamp(320, 3840),
            height: p.height.unwrap_or(900).clamp(240, 2160),
        };
        browser::screenshot(&running.url(), &p.page, viewport, &out).await?;
        Ok(format!(
            "Saved {relative} ({}x{}) of {} on Odoo {} (database {}). Place it with diagram_add_image using path \"{relative}\".",
            viewport.width, viewport.height, p.page, p.version, running.database
        ))
    }

    fn scene_path(&self) -> PathBuf {
        self.session_dir.join(SCENE_FILE)
    }

    fn load(&self) -> Result<Scene> {
        let raw =
            fs::read_to_string(self.scene_path()).context("The session has no diagram file.")?;
        Scene::parse(&raw)
    }

    /// Load, change and save the diagram as one step; nothing is written when the change fails.
    async fn edit(&self, change: impl FnOnce(&mut Scene) -> Result<String>) -> CallToolResult {
        let _guard = self.lock.lock().await;
        reply(self.load().and_then(|mut scene| {
            let message = change(&mut scene)?;
            write_atomic(&self.scene_path(), scene.to_json().as_bytes())?;
            Ok(message)
        }))
    }

    /// A file inside the session folder; anything else is refused.
    fn session_file(&self, relative: &str) -> Result<PathBuf> {
        let rel = Path::new(relative);
        ensure!(
            rel.is_relative(),
            "Use a path inside the session folder, like screenshots/x.png."
        );
        let root = self
            .session_dir
            .canonicalize()
            .context("The session folder is missing.")?;
        let path = root
            .join(rel)
            .canonicalize()
            .with_context(|| format!("{relative} does not exist."))?;
        ensure!(
            path.starts_with(&root),
            "Only files inside the session folder can be used."
        );
        Ok(path)
    }
}

/// A safe file name: lowercase letters, digits and underscores only.
fn file_stem(name: &str) -> Result<String> {
    let mut stem = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            stem.push(c);
        } else if !stem.is_empty() && !stem.ends_with('_') {
            stem.push('_');
        }
    }
    let stem: String = stem.trim_end_matches('_').chars().take(60).collect();
    ensure!(
        !stem.is_empty(),
        "Give the screenshot a name, like \"sales_order_form\"."
    );
    Ok(stem)
}

/// Tells the assistant right away when what it placed covers other text or boxes,
/// so it fixes the layout itself (e.g. with diagram_move) instead of leaving it.
fn overlap_note(scene: &Scene, id: &str) -> String {
    let others = scene.overlapping(id);
    if others.is_empty() {
        return String::new();
    }
    format!(
        " Warning: it overlaps {}; move one of them so both stay readable.",
        others.join(", ")
    )
}

/// A spot given in a screenshot's own pixels, or on the canvas.
fn target(image: Option<String>, x: f64, y: f64) -> Target {
    match image {
        Some(image) => Target::OnImage { image, x, y },
        None => Target::Point(x, y),
    }
}

fn parse_color(name: &str) -> Result<Color> {
    Color::parse(name).with_context(|| {
        format!(
            "Unknown colour \"{name}\". Use one of: {}.",
            Color::NAMES.join(", ")
        )
    })
}

/// The assistant sometimes writes a backslash followed by "n" where it means a line break.
fn unescape(text: &str) -> String {
    text.replace("\\n", "\n")
}

fn reply(result: Result<String>) -> CallToolResult {
    match result {
        Ok(message) => CallToolResult::success(vec![ContentBlock::text(message)]),
        Err(e) => CallToolResult::error(vec![ContentBlock::text(format!("{e:#}"))]),
    }
}

/// Write via a temp file + rename so the app never reads a half-written diagram.
fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let tmp = path.with_extension("excalidraw.tmp");
    fs::write(&tmp, contents).context("Could not write the diagram.")?;
    fs::rename(&tmp, path).context("Could not save the diagram.")
}

#[tool_handler(
    name = "odoo-dev-specs",
    instructions = "Tools to build the spec diagram of the current session. Elements are referenced by the ids these tools return; call diagram_summary to see what is already there."
)]
impl ServerHandler for DiagramServer {}
