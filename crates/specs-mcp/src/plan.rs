//! A whole spec described as content (sections, screens, notes, new fields, tests)
//! and laid out by code: the assistant decides what to say, never where it goes.
//! One plan replaces dozens of drawing calls, and nothing can overlap by accident.

use std::collections::HashMap;

use anyhow::{bail, ensure, Result};
use excalidraw::{Color, Placement, Scene, Target};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::anchors::{self, Area};
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
    /// Real Odoo screens of this requirement, top to bottom.
    #[serde(default)]
    pub screens: Vec<Screen>,
    /// Put the screens next to each other, when that reads better (e.g. before / after).
    #[serde(default, alias = "side_by_side")]
    pub horizontal: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Screen {
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
        for screen in &section.screens {
            for note in &screen.notes {
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
        b.link(link)?;
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
        if !section.text.is_empty() {
            y = self.text(&section.text.join("\n"), 0.0, y, BODY, None) + 12.0;
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

    /// One screenshot with its caption, model, new fields and notes at (x, y).
    /// Returns the right edge of its notes and the bottom of the block.
    fn screen(
        &mut self,
        screen: &Screen,
        shot: &ShotData,
        x: f64,
        mut y: f64,
        width: f64,
    ) -> Result<(f64, f64)> {
        if let Some(caption) = &screen.caption {
            y = self.text(caption, x, y, BODY, None) + 4.0;
        }
        if let Some(model) = &screen.model {
            y = self.text(&format!("Model: {model}"), x, y, SMALL, Some(Color::Blue)) + 6.0;
        }
        let image = self.scene.add_image(&shot.png, at(x, y), Some(width))?;
        self.scene.set_source(&image, &shot.path)?;
        self.made.push(image.clone());
        self.images.insert(screen.name.clone(), image.clone());
        let (_, iy, iw, ih) = self
            .scene
            .bounds(std::slice::from_ref(&image))
            .expect("just added");
        let notes_x = x + iw + NOTES_GAP;
        // Notes go top to bottom in their column, each level with what it is about.
        let mut free = iy;
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
        for (_, item) in items {
            let note = match item {
                Item::Field(field) => {
                    match self.new_field(&image, shot, field, notes_x, free) {
                        Ok(end) => free = end + 16.0,
                        Err(e) => self.warnings.push(format!(
                            "{}: new field {}: {e:#}",
                            screen.name, field.technical
                        )),
                    }
                    continue;
                }
                Item::Note(note) => note,
            };
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
            let want = match area {
                Some(a) => self.on_image(&image, a.x, a.y + a.h / 2.0)?.1 - LINE / 2.0,
                None => free,
            };
            let ny = want.max(free);
            let id = self.scene.add_text(
                &note.lines.join("\n"),
                at(notes_x, ny),
                Some(BODY),
                Some(Color::Red),
            );
            self.made.push(id.clone());
            free = self.bottom(&id) + 16.0;
            if let Some(a) = area {
                let tip = Target::OnImage {
                    image: image.clone(),
                    x: a.x + a.w,
                    y: a.y + a.h / 2.0,
                };
                self.made.push(self.scene.add_pointer(&id, &tip, None)?);
                if note.outline.unwrap_or(true) {
                    let corner = Target::OnImage {
                        image: image.clone(),
                        x: a.x - 4.0,
                        y: a.y - 4.0,
                    };
                    self.made.push(self.scene.add_highlight(
                        &corner,
                        (a.w + 8.0, a.h + 8.0),
                        None,
                    )?);
                }
            }
        }
        bottom = bottom.max(free);
        Ok((notes_x + NOTES_WIDTH, bottom))
    }

    /// The new field's label and example value written on the screenshot, and its red
    /// note in the notes column with an arrow from the value. Returns the note's bottom.
    fn new_field(
        &mut self,
        image: &str,
        shot: &ShotData,
        f: &NewField,
        notes_x: f64,
        free: f64,
    ) -> Result<f64> {
        let note_lines = || field_note(f);
        // Prototyped: the field is really on the screen; outline it and point at it.
        if let Ok(real) = anchors::locate(&shot.map, &format!("row:{}", f.technical))
            .or_else(|_| anchors::locate(&shot.map, &format!("column:{}", f.technical)))
        {
            let (_, my) = self.on_image(image, real.x, real.y + real.h / 2.0)?;
            let note = self.scene.add_text(
                &note_lines(),
                at(notes_x, (my - LINE / 2.0).max(free)),
                Some(BODY),
                Some(Color::Red),
            );
            self.made.push(note.clone());
            let tip = Target::OnImage {
                image: image.to_string(),
                x: real.x + real.w,
                y: real.y + real.h / 2.0,
            };
            self.made.push(self.scene.add_pointer(&note, &tip, None)?);
            let corner = Target::OnImage {
                image: image.to_string(),
                x: real.x - 4.0,
                y: real.y - 4.0,
            };
            self.made.push(self.scene.add_highlight(
                &corner,
                (real.w + 8.0, real.h + 8.0),
                None,
            )?);
            return Ok(self.bottom(&note));
        }
        let label_area = anchors::locate(&shot.map, &format!("label:{}", f.near))
            .or_else(|_| anchors::locate(&shot.map, &f.near))?;
        let field_area =
            anchors::locate(&shot.map, &format!("field:{}", f.near)).unwrap_or(label_area);
        let right = f.place.as_deref() == Some("right");
        let spot = |a: Area| {
            if right {
                (a.x + a.w + 12.0, a.y)
            } else {
                (a.x, a.y + a.h + 8.0)
            }
        };
        let (lx, ly) = spot(label_area);
        let (lx, ly) = self.on_image(image, lx, ly)?;
        let label = self.scene.add_text(&f.label, at(lx, ly), Some(SMALL), None);
        self.made.push(label.clone());
        let mut from = label;
        if let Some(value) = &f.value {
            let (vx, vy) = spot(field_area);
            let (vx, vy) = self.on_image(image, vx, vy)?;
            let v = self.scene.add_text(value, at(vx, vy), Some(SMALL), None);
            self.made.push(v.clone());
            from = v;
        }
        let ny = (ly - LINE / 2.0).max(free);
        let note = self.scene.add_text(
            &field_note(f),
            at(notes_x, ny),
            Some(BODY),
            Some(Color::Red),
        );
        self.made.push(note.clone());
        let arrow = self.scene.add_arrow(&from, &note, None)?;
        self.scene.style_line(&arrow, Color::Red, false)?;
        self.made.push(arrow);
        Ok(self.bottom(&note))
    }

    fn link(&mut self, link: &Link) -> Result<()> {
        let (Some(from), Some(to)) = (self.images.get(&link.from), self.images.get(&link.to))
        else {
            bail!(
                "The link from \"{}\" to \"{}\" names an unknown screen.",
                link.from,
                link.to
            );
        };
        let arrow = self.scene.add_arrow(from, to, link.label.as_deref())?;
        self.scene.style_line(&arrow, Color::Red, true)?;
        self.made.push(arrow);
        Ok(())
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
