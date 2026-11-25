//! A whole spec described as content (sections, screens, notes, new fields, tests)
//! and laid out by code: the assistant decides what to say, never where it goes.
//! One plan replaces dozens of drawing calls, and nothing can overlap by accident.

use std::collections::HashMap;

use anyhow::{bail, ensure, Result};
use excalidraw::{Color, Placement, Scene, Target};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::anchors;
use crate::browser::Element;

/// Elements made by the builder carry this tag, so a new plan replaces them.
pub const TAG: &str = "spec_build";

const TITLE: f64 = 36.0;
const HEADING: f64 = 28.0;
const BODY: f64 = 20.0;
const SMALL: f64 = 16.0;
const LINE: f64 = BODY * 1.25;
const SCREEN_WIDTH: f64 = 900.0;
const SIDE_SCREEN_WIDTH: f64 = 700.0;
/// From a screenshot's right edge to its notes, and the notes' width.
const NOTES_GAP: f64 = 60.0;
const NOTES_WIDTH: f64 = 360.0;
const SECTION_GAP: f64 = 100.0;
const PANEL_PADDING: f64 = 30.0;
const BLOCK_GAP: f64 = 60.0;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SpecPlan {
    /// The feature in a few words, e.g. "Delivery deadline on quotations".
    pub title: String,
    /// One line: what the client needs.
    pub need: String,
    /// Business context, 3 to 5 short lines (why, hosting, any unusual flow).
    #[serde(default)]
    pub context: Vec<String>,
    /// Odoo version for the screenshots, e.g. "19.0".
    pub version: String,
    /// Modules the screens need, e.g. ["sale_management"].
    #[serde(default)]
    pub modules: Vec<String>,
    /// New fields and models created for real on the local database just for the
    /// screenshots (removed right after), so "to be" screens show them in place.
    pub prototype: Option<crate::proto::Prototype>,
    /// The parts of the spec, top to bottom.
    pub sections: Vec<Section>,
    /// Dashed red arrows between two screens (by screen name), carrying a rule.
    #[serde(default)]
    pub links: Vec<Link>,
    /// Test cases, e.g. "Salesperson: confirms a quotation without the date -> blocked".
    #[serde(default)]
    pub tests: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Section {
    /// One requirement or business flow, in one plain frame; numbered for you,
    /// e.g. "Revise a quotation".
    pub title: String,
    /// Up to 5 short lines under the heading, in business words; numbered steps are fine.
    #[serde(default)]
    pub text: Vec<String>,
    /// A condition for the whole section, shown in orange at its top, e.g. "NOTE: only
    /// for transfers whose operation type has an approver".
    #[serde(default)]
    pub note: Option<String>,
    /// Real Odoo screens of this requirement, top to bottom.
    #[serde(default)]
    pub screens: Vec<Screen>,
    /// Put the screens next to each other, when that reads better (e.g. before / after).
    #[serde(default, alias = "side_by_side")]
    pub horizontal: bool,
    /// A mock-up of the expected result of a new report or list: its columns, where
    /// each comes from (a field or a formula), and two or three made-up rows. Drawn
    /// as a table above the screens.
    #[serde(default)]
    pub table: Option<Table>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Table {
    /// e.g. "Stock card (expected)".
    #[serde(default)]
    pub caption: Option<String>,
    pub columns: Vec<Column>,
    /// Made-up example rows, one value per column; never client data.
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Column {
    pub name: String,
    /// Where the value comes from: a field, or a formula in words
    /// ("Opening + Received - Delivered").
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Screen {
    /// The case or condition this screen shows, in orange above it, e.g. "IF APPROVED",
    /// "CASE 2: created from a purchase order", "IF Unlock = TRUE".
    #[serde(default)]
    pub condition: Option<String>,
    /// Put a copy of the record in the state this screen shows before taking it (the
    /// original demo record is never changed): call its buttons, set values, schedule
    /// activities, open a pop-up. Needs `model` and a page ending with the record id.
    #[serde(default)]
    pub scenario: Option<crate::scenario::Scenario>,
    /// Short unique name, e.g. "quotation_form"; also the screenshot's file name.
    pub name: String,
    /// Path on the local Odoo, e.g. "/odoo/sales/3" or "/report/html/sale.report_saleorder/3";
    /// for a prototype's new model "model:x_name" (its list) or "model:x_name/new".
    pub page: String,
    /// Model shown, e.g. "sale.order"; written above the screenshot.
    pub model: Option<String>,
    /// Short caption above it, e.g. "CURRENT", "EXPECTED", "Step 2: validate the delivery".
    pub caption: Option<String>,
    /// Browser size in pixels (default 1440 x 900).
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Red notes on existing parts of the screen.
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Fields, columns or buttons that do not exist yet, drawn where they will appear.
    #[serde(default)]
    pub new_fields: Vec<NewField>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Note {
    /// What the note says, which sets its colour: "new" (a new or changed thing, red,
    /// the default), "action" (what the user does, or a status, green), "result" (what
    /// the system does, blue), "condition" (orange).
    #[serde(default)]
    pub kind: Option<String>,
    /// "left" or "right" (default) of the screenshot: put a note on the side of what it
    /// points at, so arrows stay short.
    #[serde(default)]
    pub side: Option<String>,
    /// The note, one short idea per line.
    pub lines: Vec<String>,
    /// What it is about on the screenshot (e.g. "button:action_confirm", "row:validity_date",
    /// "Expiration"); the note gets an arrow to it. Leave out for a general note.
    pub anchor: Option<String>,
    /// Outline what the anchor names in red (default true).
    pub outline: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct NewField {
    /// Technical name of the existing field it goes next to, e.g. "validity_date".
    pub near: String,
    /// "below" (default) or "right" of that field.
    pub place: Option<String>,
    /// Label as the user will see it, e.g. "Delivery Deadline".
    pub label: String,
    /// Example value shown as if filled in, e.g. "12/20/2026".
    pub value: Option<String>,
    /// Technical name, e.g. "x_delivery_deadline".
    pub technical: String,
    /// Field type, e.g. "date", "many2one (res.partner)".
    #[serde(rename = "type")]
    pub field_type: String,
    /// One more short line if really needed, e.g. "read-only after confirmation".
    pub extra: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Link {
    /// Screen name the arrow starts from.
    pub from: String,
    /// Screen name it points to.
    pub to: String,
    /// The rule it carries, e.g. "partner_id = the user's partner".
    pub label: Option<String>,
    /// Start the arrow on this element of the `from` screen (e.g. the button that is
    /// clicked) instead of the screen's edge.
    #[serde(default)]
    pub from_anchor: Option<String>,
    /// End the arrow on this element of the `to` screen.
    #[serde(default)]
    pub to_anchor: Option<String>,
    /// "action" (green, default for a click), "result" (blue), "new" (red), "condition".
    #[serde(default)]
    pub kind: Option<String>,
}

/// A screenshot ready to place: its file (inside the session), the PNG and its map.
pub struct ShotData {
    pub path: String,
    pub png: Vec<u8>,
    pub map: Vec<Element>,
}

/// What was built, and what could not be done as asked.
#[derive(Debug, Default)]
pub struct Report {
    pub elements: usize,
    pub warnings: Vec<String>,
}

/// Plan mistakes found before any screenshot is taken.
pub fn validate(plan: &SpecPlan) -> Result<()> {
    ensure!(!plan.sections.is_empty(), "Give at least one section.");
    if let Some(p) = &plan.prototype {
        p.validate()?;
    }
    let new_models: Vec<&str> = plan
        .prototype
        .iter()
        .flat_map(|p| &p.models)
        .map(|m| m.model.as_str())
        .collect();
    let mut names = Vec::new();
    for screen in plan.sections.iter().flat_map(|s| &s.screens) {
        ensure!(
            !names.contains(&screen.name),
            "Two screens are called \"{}\": give each a unique name.",
            screen.name
        );
        names.push(screen.name.clone());
        if let Some(sc) = &screen.scenario {
            crate::scenario::validate(screen.model.as_deref(), &screen.page, sc)?;
        }
        if let Some(model) = screen.page.strip_prefix("model:") {
            let model = model.strip_suffix("/new").unwrap_or(model);
            ensure!(
                new_models.contains(&model),
                "Screen \"{}\" shows {model}, which is not a new model of the prototype.",
                screen.name
            );
        }
        for f in &screen.new_fields {
            if let Some(p) = f.place.as_deref() {
                ensure!(
                    p == "below" || p == "right",
                    "place must be \"below\" or \"right\", not \"{p}\"."
                );
            }
        }
    }
    for link in &plan.links {
        if let Some(k) = link.kind.as_deref() {
            ensure!(
                kind_color(k).is_some(),
                "A link has kind \"{k}\": use new, action, result or condition."
            );
        }
        for end in [&link.from, &link.to] {
            ensure!(
                names.contains(end),
                "The link refers to \"{end}\", which is not a screen name."
            );
        }
    }
    for section in &plan.sections {
        ensure!(
            section.text.len() <= MAX_TEXT_LINES,
            "Section \"{}\" has {} lines of text: keep it to {MAX_TEXT_LINES}.",
            section.title,
            section.text.len()
        );
        plain(&section.text)?;
        if let Some(t) = &section.table {
            ensure!(
                !t.columns.is_empty(),
                "The table of \"{}\" has no column.",
                section.title
            );
            for row in &t.rows {
                ensure!(
                    row.len() == t.columns.len(),
                    "A row of the table of \"{}\" has {} values for {} columns: give one per column.",
                    section.title,
                    row.len(),
                    t.columns.len()
                );
            }
        }
        for screen in &section.screens {
            for note in &screen.notes {
                if let Some(k) = note.kind.as_deref() {
                    ensure!(kind_color(k).is_some(), "A note on screen \"{}\" has kind \"{k}\": use new, action, result or condition.", screen.name);
                }
                if let Some(side) = note.side.as_deref() {
                    ensure!(
                        side == "left" || side == "right",
                        "A note on screen \"{}\" has side \"{side}\": use left or right.",
                        screen.name
                    );
                }
                ensure!(
                    note.lines.len() <= MAX_NOTE_LINES,
                    "A note on screen \"{}\" has {} lines: keep each note to {MAX_NOTE_LINES} lines (split it or shorten it).",
                    screen.name,
                    note.lines.len()
                );
                plain(&note.lines)?;
            }
        }
    }
    plain(&plan.context)?;
    Ok(())
}

const MAX_NOTE_LINES: usize = 3;
/// A note pushed further than this below what it points at counts as crowded.
const CROWDED: f64 = 40.0;

/// The colour of a note or arrow of this kind.
pub fn kind_color(kind: &str) -> Option<Color> {
    match kind {
        "new" => Some(Color::Red),
        "action" | "status" => Some(Color::Green),
        "result" => Some(Color::Blue),
        "condition" | "case" => Some(Color::Yellow),
        _ => None,
    }
}

/// The screenshot's height in its own pixels (the PNG header), to map map points.
fn shot_height(shot: &ShotData) -> f64 {
    shot.png
        .get(20..24)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64)
        .filter(|h| *h > 0.0)
        .unwrap_or(1.0)
}
const MAX_TEXT_LINES: usize = 5;

/// Business words, not code: the reader is a business analyst and a developer who
/// knows Odoo, so "cancel this quotation", not "action_cancel()".
fn plain(lines: &[String]) -> Result<()> {
    for line in lines {
        let code = line.contains("()")
            || line.contains("record.")
            || line.contains("self.")
            || line.contains("env[");
        ensure!(
            !code,
            "\"{line}\" reads like code: write it in business words (e.g. \"cancel this quotation\")."
        );
    }
    Ok(())
}

/// Every screen of the plan, in order.
/// New fields the plan lists on a screen but did not prototype are prototyped all the
/// same, so they are really on the screenshot (never text written over it). Only for
/// a screen whose model is known, and types that can be made as data.
pub fn auto_prototype(plan: &mut SpecPlan) {
    let mut added = Vec::new();
    for screen in plan.sections.iter().flat_map(|s| &s.screens) {
        let Some(model) = screen.model.as_deref().filter(|m| !m.is_empty()) else {
            continue;
        };
        for f in &screen.new_fields {
            let mut words = f
                .field_type
                .split([' ', '(', ')'])
                .filter(|w| !w.is_empty());
            let kind = words.next().unwrap_or_default().to_lowercase();
            let relation = words.next().map(String::from);
            if !crate::proto::can_make(&kind) || (kind.starts_with("many2") && relation.is_none()) {
                continue;
            }
            added.push(crate::proto::ProtoField {
                model: model.to_string(),
                name: proto_name(&f.technical),
                label: f.label.clone(),
                selection: if kind == "selection" {
                    vec![f.value.clone().unwrap_or_else(|| "Value".into())]
                } else {
                    vec![]
                },
                field_type: kind,
                relation,
                after: Some(f.near.clone()),
                list_after: None,
            });
        }
    }
    let proto = plan.prototype.get_or_insert_with(Default::default);
    for f in added {
        if !proto
            .fields
            .iter()
            .any(|p| p.name == f.name && p.model == f.model)
        {
            proto.fields.push(f);
        }
    }
}

/// The prototype's name for a new field: Studio-like data fields start with x_.
fn proto_name(technical: &str) -> String {
    let clean: String = technical
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.starts_with("x_") {
        clean
    } else {
        format!("x_{clean}")
    }
}

pub fn screens(plan: &SpecPlan) -> Vec<&Screen> {
    plan.sections.iter().flat_map(|s| &s.screens).collect()
}

/// Replace what an earlier plan built with the layout of this one. Elements the user
/// or the assistant added by hand are kept (below the new spec when they were there).
pub fn build(
    scene: &mut Scene,
    plan: &SpecPlan,
    shots: &HashMap<String, ShotData>,
) -> Result<Report> {
    validate(plan)?;
    let old = scene.tagged(TAG);
    if !old.is_empty() {
        scene.delete(&old)?;
    }
    let mut b = Builder {
        scene,
        made: Vec::new(),
        images: HashMap::new(),
        warnings: Vec::new(),
    };
    let mut y = 0.0;
    y = b.text(&plan.title, 0.0, y, TITLE, None) + 12.0;
    y = b.text(&plan.need, 0.0, y, BODY, None) + 24.0;
    if !plan.context.is_empty() {
        y = b.text("Business context", 0.0, y, HEADING, None) + 12.0;
        y = b.text(&plan.context.join("\n"), 0.0, y, BODY, None);
    }
    for (n, section) in plan.sections.iter().enumerate() {
        let framed = plan.sections.len() > 1;
        y = b.section(n + 1, section, y + SECTION_GAP, shots, framed)?;
    }
    for link in &plan.links {
        b.link(link, shots)?;
    }
    if !plan.tests.is_empty() {
        y = b.text("Test cases", 0.0, y + SECTION_GAP, HEADING, None) + 12.0;
        let lines: Vec<String> = plan
            .tests
            .iter()
            .enumerate()
            .map(|(i, t)| format!("[ ] {}. {}", i + 1, t.trim()))
            .collect();
        b.text(&lines.join("\n"), 0.0, y, BODY, None);
    }
    for id in b.made.clone() {
        b.scene.tag(&id, TAG)?;
    }
    b.check_layout();
    for id in &b.made {
        let hits = b.scene.overlapping(id);
        if !hits.is_empty() {
            b.warnings.push(format!(
                "{id} overlaps {}: shorten that text.",
                hits.join(", ")
            ));
        }
    }
    Ok(Report {
        elements: b.made.len(),
        warnings: b.warnings,
    })
}

enum Item<'a> {
    Field(&'a NewField),
    Note(&'a Note),
}

/// Where a new field is on the screenshot (its real place once prototyped, else the
/// field it goes next to); unknown ones go last.
fn field_y(shot: &ShotData, f: &NewField) -> f64 {
    [
        format!("row:{}", f.technical),
        format!("column:{}", f.technical),
        format!("label:{}", f.near),
        f.near.clone(),
    ]
    .iter()
    .find_map(|a| anchors::locate(&shot.map, a).ok())
    .map_or(f64::INFINITY, |a| a.y)
}

struct Builder<'a> {
    scene: &'a mut Scene,
    made: Vec<String>,
    /// Screen name -> its image element.
    images: HashMap<String, String>,
    warnings: Vec<String>,
}

impl Builder<'_> {
    /// Write text at (x, y); returns the y below it.
    fn text(&mut self, content: &str, x: f64, y: f64, size: f64, color: Option<Color>) -> f64 {
        let id = self.scene.add_text(content, at(x, y), Some(size), color);
        self.made.push(id.clone());
        self.bottom(&id)
    }

    fn bottom(&self, id: &str) -> f64 {
        self.scene
            .bounds(&[id.to_string()])
            .map(|(_, y, _, h)| y + h)
            .unwrap_or_default()
    }

    fn section(
        &mut self,
        n: usize,
        section: &Section,
        top: f64,
        shots: &HashMap<String, ShotData>,
        framed: bool,
    ) -> Result<f64> {
        let first = self.made.len();
        let mut y = self.text(&format!("{n}. {}", section.title), 0.0, top, HEADING, None) + 12.0;
        if let Some(note) = &section.note {
            y = self.text(note, 0.0, y, BODY, Some(Color::Yellow)) + 12.0;
        }
        if !section.text.is_empty() {
            y = self.text(&section.text.join("\n"), 0.0, y, BODY, None) + 12.0;
        }
        if let Some(table) = &section.table {
            y = self.table(table, 0.0, y) + BLOCK_GAP;
        }
        y += 20.0;
        let mut x = 0.0;
        let mut bottom = y;
        for screen in &section.screens {
            let width = if section.horizontal {
                SIDE_SCREEN_WIDTH
            } else {
                SCREEN_WIDTH
            };
            let shot = shots.get(&screen.name).ok_or_else(|| {
                anyhow::anyhow!("There is no screenshot for screen \"{}\".", screen.name)
            })?;
            let (right, end) = self.screen(screen, shot, x, y, width)?;
            bottom = bottom.max(end);
            if section.horizontal {
                x = right + BLOCK_GAP;
            } else {
                y = end + BLOCK_GAP;
            }
        }
        let ids = self.made[first..].to_vec();
        let (bx, by, bw, bh) = self.scene.bounds(&ids).unwrap_or((0.0, top, 0.0, 0.0));
        // A frame only tells requirements apart: one requirement needs none.
        if !framed {
            return Ok(by + bh);
        }
        let panel = self.scene.add_outline((
            bx - PANEL_PADDING,
            by - PANEL_PADDING,
            bw + 2.0 * PANEL_PADDING,
            bh + 2.0 * PANEL_PADDING,
        ));
        self.made.push(panel);
        Ok(by + bh + PANEL_PADDING)
    }

    /// A mock-up table at (x, y): the caption, a header row with each column's name
    /// and source, then the rows, as a grid. Returns the y below it.
    fn table(&mut self, table: &Table, x: f64, mut y: f64) -> f64 {
        const PAD: f64 = 12.0;
        let chars = |t: &str, size: f64| {
            t.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * size * 0.55
        };
        if let Some(caption) = &table.caption {
            y = self.text(caption, x, y, BODY, None) + 10.0;
        }
        let widths: Vec<f64> = table
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let cells = table
                    .rows
                    .iter()
                    .map(|r| chars(&r[i], BODY))
                    .fold(0.0, f64::max);
                let source = c.source.as_deref().map_or(0.0, |s| chars(s, SMALL));
                (chars(&c.name, BODY).max(cells).max(source) + 2.0 * PAD).clamp(110.0, 320.0)
            })
            .collect();
        let has_source = table.columns.iter().any(|c| c.source.is_some());
        let source_lines = table
            .columns
            .iter()
            .zip(&widths)
            .filter_map(|(c, w)| {
                let fits = (((w - 2.0 * PAD) / (SMALL * 0.55)) as usize).max(8);
                c.source.as_deref().map(|s| wrap(s, fits).lines().count())
            })
            .max()
            .unwrap_or(0) as f64;
        let header = LINE
            + 2.0 * PAD
            + if has_source {
                4.0 + source_lines * SMALL * 1.3
            } else {
                0.0
            };
        let mut cx = x;
        for (c, w) in table.columns.iter().zip(&widths) {
            let cell = self.scene.add_outline((cx, y, *w, header));
            self.made.push(cell);
            self.text(&c.name, cx + PAD, y + PAD, BODY, None);
            if let Some(source) = &c.source {
                let fits = (((w - 2.0 * PAD) / (SMALL * 0.55)) as usize).max(8);
                let id = self.scene.add_text(
                    &wrap(source, fits),
                    at(cx + PAD, y + PAD + LINE + 4.0),
                    Some(SMALL),
                    Some(Color::Blue),
                );
                self.made.push(id);
            }
            cx += w;
        }
        y += header;
        for row in &table.rows {
            let mut cx = x;
            for (value, w) in row.iter().zip(&widths) {
                let cell = self.scene.add_outline((cx, y, *w, LINE + 2.0 * PAD));
                self.made.push(cell);
                self.text(value, cx + PAD, y + PAD, BODY, None);
                cx += w;
            }
            y += LINE + 2.0 * PAD;
        }
        y
    }

    /// One screenshot with its condition, caption, model, new fields and notes at
    /// (x, y): notes in a column on its right, and on its left for those asked there.
    /// Returns the right edge of what it drew and the bottom of the block.
    fn screen(
        &mut self,
        screen: &Screen,
        shot: &ShotData,
        x: f64,
        mut y: f64,
        width: f64,
    ) -> Result<(f64, f64)> {
        let first = self.made.len();
        let on_left =
            |item: &Item| matches!(item, Item::Note(n) if n.side.as_deref() == Some("left"));
        let left_width = screen
            .notes
            .iter()
            .filter(|n| n.side.as_deref() == Some("left"))
            .map(|n| text_width(&n.lines.join("\n"), BODY))
            .fold(0.0, f64::max);
        let ix = if left_width > 0.0 {
            x + left_width + NOTES_GAP
        } else {
            x
        };
        if let Some(condition) = &screen.condition {
            y = self.text(condition, ix, y, BODY, Some(Color::Yellow)) + 6.0;
        }
        if let Some(caption) = &screen.caption {
            y = self.text(caption, ix, y, BODY, None) + 4.0;
        }
        if let Some(model) = &screen.model {
            y = self.text(&format!("Model: {model}"), ix, y, SMALL, Some(Color::Blue)) + 6.0;
        }
        let image = self.scene.add_image(&shot.png, at(ix, y), Some(width))?;
        self.scene.set_source(&image, &shot.path)?;
        self.made.push(image.clone());
        self.images.insert(screen.name.clone(), image.clone());
        let (_, iy, iw, ih) = self
            .scene
            .bounds(std::slice::from_ref(&image))
            .expect("just added");
        let right_x = ix + iw + NOTES_GAP;
        let mut free = [iy, iy]; // [right column, left column]
        let mut bottom = iy + ih;

        // Top to bottom in the order of what they point at, so arrows do not cross.
        let mut items: Vec<(f64, Item)> = screen
            .new_fields
            .iter()
            .map(|f| (field_y(shot, f), Item::Field(f)))
            .chain(screen.notes.iter().map(|n| {
                let y = n
                    .anchor
                    .as_deref()
                    .and_then(|a| anchors::locate(&shot.map, a).ok())
                    .map_or(f64::INFINITY, |a| a.y);
                (y, Item::Note(n))
            }))
            .collect();
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        // When notes would have to be pushed far below what they point at (several
        // things on one line, e.g. header buttons), long arrows would cross each other:
        // number them on the screenshot instead, with no arrows. Each side on its own.
        let crowded = |left: bool| {
            let mut next = iy;
            items
                .iter()
                .filter(|(_, i)| on_left(i) == left)
                .any(|(at, item)| {
                    if !at.is_finite() {
                        return false;
                    }
                    let want = iy + at * (ih / shot_height(shot)) - LINE / 2.0;
                    let pushed = next - want > CROWDED;
                    let lines = match item {
                        Item::Field(f) => field_note(f).lines().count() + 1,
                        Item::Note(n) => n.lines.len(),
                    };
                    next = want.max(next) + lines as f64 * LINE + 16.0;
                    pushed
                })
        };
        let numbered = [crowded(false), crowded(true)];
        let mut number = 0;
        for (_, item) in &items {
            let left = on_left(item);
            let side = usize::from(left);
            let (lines, area, outline, color) = match item {
                Item::Field(field) => {
                    let found = anchors::locate(&shot.map, &format!("row:{}", field.technical))
                        .or_else(|_| {
                            anchors::locate(&shot.map, &format!("column:{}", field.technical))
                        })
                        .or_else(|_| {
                            anchors::locate(
                                &shot.map,
                                &format!("row:{}", proto_name(&field.technical)),
                            )
                        })
                        .or_else(|_| {
                            anchors::locate(
                                &shot.map,
                                &format!("column:{}", proto_name(&field.technical)),
                            )
                        });
                    let area = match found {
                        Ok(real) => Some(real),
                        // Not prototyped: point at the field it goes next to.
                        Err(_) => {
                            match anchors::locate(&shot.map, &format!("field:{}", field.near))
                                .or_else(|_| {
                                    anchors::locate(&shot.map, &format!("label:{}", field.near))
                                })
                                .or_else(|_| anchors::locate(&shot.map, &field.near))
                            {
                                Ok(near) => Some(near),
                                Err(e) => {
                                    self.warnings.push(format!(
                                        "{}: new field {}: {e:#}",
                                        screen.name, field.technical
                                    ));
                                    None
                                }
                            }
                        }
                    };
                    let mut text = field_note(field);
                    if found.is_err() && area.is_some() {
                        let place = if field.place.as_deref() == Some("right") {
                            "right of"
                        } else {
                            "below"
                        };
                        text.push_str(&format!("\n(goes {place} this field)"));
                    }
                    (text, area, true, Color::Red)
                }
                Item::Note(note) => {
                    let area = match &note.anchor {
                        Some(anchor) => match anchors::locate(&shot.map, anchor) {
                            Ok(a) => Some(a),
                            Err(e) => {
                                self.warnings.push(format!("{}: {e:#}", screen.name));
                                None
                            }
                        },
                        None => None,
                    };
                    let color = note
                        .kind
                        .as_deref()
                        .and_then(kind_color)
                        .unwrap_or(Color::Red);
                    (
                        note.lines.join("\n"),
                        area,
                        note.outline.unwrap_or(true),
                        color,
                    )
                }
            };
            let want = match area {
                Some(a) => self.on_image(&image, a.x, a.y + a.h / 2.0)?.1 - LINE / 2.0,
                None => free[side],
            };
            let text = match area {
                Some(_) if numbered[side] => {
                    number += 1;
                    format!("{number}. {lines}")
                }
                _ => lines,
            };
            let nx = if left {
                ix - NOTES_GAP - text_width(&text, BODY)
            } else {
                right_x
            };
            let id =
                self.scene
                    .add_text(&text, at(nx, want.max(free[side])), Some(BODY), Some(color));
            self.made.push(id.clone());
            free[side] = self.bottom(&id) + 16.0;
            let Some(a) = area else { continue };
            if numbered[side] {
                let mark_x = if left { a.x - 26.0 } else { a.x + a.w + 6.0 };
                let (mx, my) = self.on_image(&image, mark_x, a.y - 6.0)?;
                let marker = self.scene.add_text(
                    &number.to_string(),
                    at(mx, my - LINE / 2.0),
                    Some(BODY),
                    Some(color),
                );
                self.made.push(marker);
            } else {
                let tip = Target::OnImage {
                    image: image.clone(),
                    x: if left { a.x } else { a.x + a.w },
                    y: a.y + a.h / 2.0,
                };
                self.made
                    .push(self.scene.add_pointer(&id, &tip, Some(color))?);
            }
            if outline {
                let corner = Target::OnImage {
                    image: image.clone(),
                    x: a.x - 4.0,
                    y: a.y - 4.0,
                };
                self.made.push(self.scene.add_highlight(
                    &corner,
                    (a.w + 8.0, a.h + 8.0),
                    Some(color),
                )?);
            }
        }
        bottom = bottom.max(free[0]).max(free[1]);
        // The right edge of what this screen drew: a long note may be wider than
        // the notes column, and the next screen of a row must clear it.
        let right = self
            .scene
            .bounds(&self.made[first..])
            .map_or(0.0, |(bx, _, bw, _)| bx + bw)
            .max(right_x + NOTES_WIDTH);
        Ok((right, bottom))
    }

    fn link(&mut self, link: &Link, shots: &HashMap<String, ShotData>) -> Result<()> {
        let (Some(from), Some(to)) = (
            self.images.get(&link.from).cloned(),
            self.images.get(&link.to).cloned(),
        ) else {
            bail!(
                "The link from \"{}\" to \"{}\" names an unknown screen.",
                link.from,
                link.to
            );
        };
        let color = link.kind.as_deref().and_then(kind_color);
        if link.from_anchor.is_none() && link.to_anchor.is_none() {
            let arrow = self.scene.add_arrow(&from, &to, link.label.as_deref())?;
            self.scene
                .style_line(&arrow, color.unwrap_or(Color::Red), true)?;
            self.made.push(arrow);
            return Ok(());
        }
        // From an element of one screen (the button clicked) to the next screen: the
        // element is outlined, and the arrow runs from the screenshot's edge level with
        // it to the next screenshot's edge, so it never crosses a screenshot.
        let color = color.unwrap_or(Color::Green);
        let rect =
            |this: &Self, image: &str| this.scene.bounds(&[image.to_string()]).unwrap_or_default();
        let (fx, fy, fw, fh) = rect(self, &from);
        let (tx, ty, tw, th) = rect(self, &to);
        let spot = |this: &mut Self,
                    image: &str,
                    screen: &str,
                    anchor: Option<&str>|
         -> Result<Option<(f64, f64)>> {
            let Some(name) = anchor else { return Ok(None) };
            let Some(a) = shots
                .get(screen)
                .and_then(|s| anchors::locate(&s.map, name).ok())
            else {
                this.warnings.push(format!(
                    "link {} -> {}: \"{name}\" is not on screen {screen}.",
                    link.from, link.to
                ));
                return Ok(None);
            };
            let corner = Target::OnImage {
                image: image.to_string(),
                x: a.x - 4.0,
                y: a.y - 4.0,
            };
            let outline = this
                .scene
                .add_highlight(&corner, (a.w + 8.0, a.h + 8.0), Some(color))?;
            this.made.push(outline);
            Ok(Some(this.on_image(
                image,
                a.x + a.w / 2.0,
                a.y + a.h / 2.0,
            )?))
        };
        let start_at = spot(self, &from, &link.from, link.from_anchor.as_deref())?;
        let end_at = spot(self, &to, &link.to, link.to_anchor.as_deref())?;
        let beside = tx >= fx + fw;
        let (start, end) = if beside {
            // Side by side: right edge of the first, left edge of the next.
            let sy = start_at.map_or(fy + fh / 2.0, |p| p.1);
            let ey = end_at.map_or(sy.clamp(ty, ty + th), |p| p.1);
            (
                Target::Point(fx + fw + 6.0, sy),
                Target::Point(tx - 6.0, ey),
            )
        } else {
            // One below the other: bottom edge of the first, top edge of the next.
            let sx = start_at.map_or(fx + fw / 2.0, |p| p.0);
            let ex = end_at.map_or(sx.clamp(tx, tx + tw), |p| p.0);
            (
                Target::Point(sx, fy + fh + 6.0),
                Target::Point(ex, ty - 6.0),
            )
        };
        // A screenshot between the two would be crossed: go around, below them all.
        let resolve = |t: &Target| match t {
            Target::Point(x, y) => (*x, *y),
            Target::OnImage { .. } => (0.0, 0.0),
        };
        let (p, q) = (resolve(&start), resolve(&end));
        let others: Vec<(f64, f64, f64, f64)> = self
            .images
            .values()
            .filter(|id| **id != from && **id != to)
            .filter_map(|id| self.scene.bounds(std::slice::from_ref(id)))
            .collect();
        let crosses = |r: &(f64, f64, f64, f64)| {
            let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
            let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
            r.0 < x1 && r.0 + r.2 > x0 && r.1 < y1.max(y0 + 1.0) && r.1 + r.3 > y0
        };
        let blocked: Vec<_> = others.iter().filter(|r| crosses(r)).collect();
        let (arrow, label) = if blocked.is_empty() {
            self.scene
                .add_flow(&start, &end, color, link.label.as_deref())?
        } else {
            let below = blocked
                .iter()
                .map(|r| r.1 + r.3)
                .chain([fy + fh, ty + th])
                .fold(f64::MIN, f64::max)
                + 40.0;
            let sx = start_at.map_or(fx + fw / 2.0, |p| p.0);
            let ex = end_at.map_or(tx + tw / 2.0, |p| p.0);
            self.scene.add_flow_via(
                &Target::Point(sx, fy + fh + 6.0),
                &[(sx, below), (ex, below)],
                &Target::Point(ex, ty + th + 6.0),
                color,
                link.label.as_deref(),
            )?
        };
        self.made.push(arrow);
        self.made.extend(label);
        Ok(())
    }

    /// What a reader would trip over, as warnings to fix: a note covering a
    /// screenshot, an arrow crossing a screenshot it does not point into.
    fn check_layout(&mut self) {
        let doc: serde_json::Value =
            serde_json::from_str(&self.scene.to_json()).unwrap_or_default();
        let els: Vec<&serde_json::Value> = doc["elements"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|e| {
                        e["isDeleted"] != true && self.made.iter().any(|m| e["id"] == m.as_str())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let num = |e: &serde_json::Value, k: &str| e[k].as_f64().unwrap_or(0.0);
        let screens: Vec<(String, (f64, f64, f64, f64))> = self
            .images
            .iter()
            .filter_map(|(name, id)| {
                Some((name.clone(), self.scene.bounds(std::slice::from_ref(id))?))
            })
            .collect();
        let inside = |(x, y): (f64, f64), r: &(f64, f64, f64, f64)| {
            x > r.0 + 2.0 && x < r.0 + r.2 - 2.0 && y > r.1 + 2.0 && y < r.1 + r.3 - 2.0
        };
        let mut found = Vec::new();
        for e in &els {
            match e["type"].as_str() {
                Some("text") => {
                    let text = e["text"].as_str().unwrap_or_default();
                    // Numbers on a screenshot are put there on purpose.
                    if text.len() <= 3 && text.chars().all(|c| c.is_ascii_digit()) {
                        continue;
                    }
                    let (x, y, w, h) =
                        (num(e, "x"), num(e, "y"), num(e, "width"), num(e, "height"));
                    for (name, r) in &screens {
                        if x < r.0 + r.2 && x + w > r.0 && y < r.1 + r.3 && y + h > r.1 {
                            let start: String = text.chars().take(30).collect();
                            found.push(format!("The text \"{start}\" covers screenshot {name}: shorten it, or put the note on the other `side`."));
                        }
                    }
                }
                Some("arrow") => {
                    let (x0, y0) = (num(e, "x"), num(e, "y"));
                    let points: Vec<(f64, f64)> = e["points"]
                        .as_array()
                        .map(|p| {
                            p.iter()
                                .map(|q| {
                                    (
                                        x0 + q[0].as_f64().unwrap_or(0.0),
                                        y0 + q[1].as_f64().unwrap_or(0.0),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let (Some(first), Some(last)) = (points.first(), points.last()) else {
                        continue;
                    };
                    for (name, r) in &screens {
                        if inside(*first, r) || inside(*last, r) {
                            continue;
                        }
                        let crosses = points.windows(2).any(|w| {
                            (1..20).any(|i| {
                                let t = i as f64 / 20.0;
                                inside(
                                    (
                                        w[0].0 + (w[1].0 - w[0].0) * t,
                                        w[0].1 + (w[1].1 - w[0].1) * t,
                                    ),
                                    r,
                                )
                            })
                        });
                        if crosses {
                            found.push(format!("An arrow crosses screenshot {name}: link from a button with `from_anchor`, or put the screens in another order."));
                        }
                    }
                }
                _ => {}
            }
        }
        found.sort();
        found.dedup();
        self.warnings.extend(found);
    }

    fn on_image(&self, image: &str, x: f64, y: f64) -> Result<(f64, f64)> {
        self.scene.resolve(&Target::OnImage {
            image: image.to_string(),
            x,
            y,
        })
    }
}

/// "New field: Delivery Deadline (x_delivery_deadline, date)" and, if given, one
/// more short line. The example value is on the screenshot, not repeated here.
/// About how wide a text is drawn at this size.
fn text_width(text: &str, size: f64) -> f64 {
    text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * size * 0.55
}

/// Break `text` into lines of at most `width` characters, at spaces.
fn wrap(text: &str, width: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines.join("\n")
}

fn field_note(f: &NewField) -> String {
    let mut lines = vec![format!(
        "New field: {} ({}, {})",
        f.label, f.technical, f.field_type
    )];
    lines.extend(f.extra.clone());
    lines.join("\n")
}

fn at(x: f64, y: f64) -> Placement {
    Placement {
        x: Some(x),
        y: Some(y),
    }
}
