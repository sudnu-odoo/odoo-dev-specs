//! Reading and editing `.excalidraw` documents, for the assistant's diagram tools.
//!
//! Elements are kept as JSON so fields this crate does not know survive untouched.
//! Every change bumps the element's `version`, and deletions only set `isDeleted`,
//! so the UI can merge the assistant's edits with the user's by version.

mod color;
mod png;
mod text;

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{ensure, Context, Result};
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Map, Value};

pub use color::Color;

/// Space left between an automatically placed element and the content above it.
const GAP: f64 = 40.0;
const MIN_BOX: (f64, f64) = (160.0, 80.0);
const MAX_BOX_WIDTH: f64 = 400.0;
const BOX_PADDING: f64 = 20.0;
/// Least space above and below a label before its box must grow (Excalidraw's own value).
const LABEL_PADDING_Y: f64 = 5.0;
const MAX_IMAGE_WIDTH: f64 = 800.0;
/// Space between an arrow end and the element it is attached to.
const ARROW_GAP: f64 = 8.0;
const INK: &str = "#1e1e1e";

/// What the assistant sees of the diagram.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementSummary {
    pub id: String,
    pub kind: String,
    /// Label of a box or arrow, text of a text element, name of a frame.
    pub text: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub frame: Option<String>,
}

/// A spot on the canvas: either canvas coordinates, or a pixel of a screenshot as the
/// assistant sees it in the PNG file (scaled to where the image is shown).
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Point(f64, f64),
    OnImage { image: String, x: f64, y: f64 },
}

/// Where to put a new element. Missing coordinates mean "below everything".
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Placement {
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn of(e: &Value) -> Rect {
        let f = |k: &str| e[k].as_f64().unwrap_or(0.0);
        Rect {
            x: f("x"),
            y: f("y"),
            w: f("width"),
            h: f("height"),
        }
    }
    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    doc: Value,
}

impl Scene {
    pub fn parse(json: &str) -> Result<Scene> {
        let doc: Value = serde_json::from_str(json).context("The diagram is not valid JSON.")?;
        ensure!(
            doc["type"] == "excalidraw",
            "The diagram is not an Excalidraw document."
        );
        ensure!(
            doc["elements"].is_array(),
            "The diagram has no element list."
        );
        let mut scene = Scene { doc };
        if !scene.doc["files"].is_object() {
            scene.doc["files"] = json!({});
        }
        Ok(scene)
    }

    pub fn to_json(&self) -> String {
        self.doc.to_string()
    }

    /// Live (not deleted) elements; a label is reported on its box or arrow.
    pub fn summary(&self) -> Vec<ElementSummary> {
        self.live()
            .filter(|e| !is_label(e))
            .map(|e| {
                let r = Rect::of(e);
                let text = match e["type"].as_str() {
                    Some("text") => e["text"].as_str().map(String::from),
                    Some("frame") => e["name"].as_str().map(String::from),
                    _ => self
                        .label_of(str_of(e, "id"))
                        .map(|l| str_of(l, "originalText").to_string()),
                };
                ElementSummary {
                    id: str_of(e, "id").to_string(),
                    kind: str_of(e, "type").to_string(),
                    text,
                    x: r.x,
                    y: r.y,
                    width: r.w,
                    height: r.h,
                    frame: e["frameId"].as_str().map(String::from),
                }
            })
            .collect()
    }

    /// Rectangle with a centred label. Returns the box id.
    pub fn add_box(
        &mut self,
        label: &str,
        at: Placement,
        size: Option<(f64, f64)>,
        color: Option<Color>,
        frame: Option<&str>,
    ) -> Result<String> {
        if let Some(frame) = frame {
            ensure!(
                self.find_live(frame).is_some_and(|e| e["type"] == "frame"),
                "There is no frame with id {frame}."
            );
        }
        let max_text_width = size.map_or(MAX_BOX_WIDTH, |(w, _)| w) - 2.0 * BOX_PADDING;
        let wrapped = text::wrap(label, max_text_width, text::FONT_SIZE);
        let (tw, th) = text::measure(&wrapped, text::FONT_SIZE);
        let (w, h) = match size {
            // Keep the requested size; only grow when the label would not fit.
            Some((w, h)) => (w, h.max(th + 2.0 * LABEL_PADDING_Y)),
            None => (
                (tw + 2.0 * BOX_PADDING).clamp(MIN_BOX.0, MAX_BOX_WIDTH),
                (th + 2.0 * BOX_PADDING).max(MIN_BOX.1),
            ),
        };
        let (x, y) = self.position(at);

        let mut rect = base("rectangle", x, y, w, h);
        rect["roundness"] = json!({ "type": 3 });
        rect["frameId"] = json!(frame);
        if let Some(c) = color {
            rect["backgroundColor"] = json!(c.fill());
            rect["strokeColor"] = json!(c.stroke());
        }
        let id = str_of(&rect, "id").to_string();
        let label = label_element(&wrapped, label, &id, Rect { x, y, w, h }, frame);
        rect["boundElements"] = json!([{ "type": "text", "id": label["id"] }]);
        self.push(rect);
        self.push(label);
        Ok(id)
    }

    /// Free text (titles, notes); `color` makes it e.g. a red annotation. Returns the text id.
    pub fn add_text(
        &mut self,
        content: &str,
        at: Placement,
        font_size: Option<f64>,
        color: Option<Color>,
    ) -> String {
        let size = font_size.unwrap_or(text::FONT_SIZE);
        let (w, h) = text::measure(content, size);
        let (x, y) = self.position(at);
        let mut e = base("text", x, y, w, h);
        if let Some(c) = color {
            e["strokeColor"] = json!(c.stroke());
        }
        set_text_fields(&mut e, content, content, size, "left", "top", None);
        let id = str_of(&e, "id").to_string();
        self.push(e);
        id
    }

    /// Arrow bound to two elements, optionally labelled. Returns the arrow id.
    pub fn add_arrow(&mut self, from: &str, to: &str, label: Option<&str>) -> Result<String> {
        for end in [from, to] {
            ensure!(
                self.find_live(end).is_some(),
                "There is no element with id {end}."
            );
        }
        let mut arrow = base("arrow", 0.0, 0.0, 0.0, 0.0);
        arrow["roundness"] = json!({ "type": 2 });
        arrow["startBinding"] = json!({ "elementId": from, "focus": 0, "gap": ARROW_GAP });
        arrow["endBinding"] = json!({ "elementId": to, "focus": 0, "gap": ARROW_GAP });
        arrow["startArrowhead"] = Value::Null;
        arrow["endArrowhead"] = json!("arrow");
        arrow["lastCommittedPoint"] = Value::Null;
        arrow["elbowed"] = json!(false);
        let id = str_of(&arrow, "id").to_string();
        self.push(arrow);
        self.route_arrow(&id);

        if let Some(label) = label {
            let r = Rect::of(self.find(&id).expect("just added"));
            let e = label_element(label, label, &id, r, None);
            let label_id = e["id"].clone();
            self.push(e);
            self.find_mut(&id).expect("just added")["boundElements"] =
                json!([{ "type": "text", "id": label_id }]);
        }
        for end in [from, to] {
            let e = self.find_mut(end).expect("checked above");
            let mut bound = e["boundElements"].as_array().cloned().unwrap_or_default();
            bound.push(json!({ "type": "arrow", "id": id }));
            e["boundElements"] = json!(bound);
            bump(e);
        }
        Ok(id)
    }

    /// Canvas coordinates of a target. A screenshot pixel is scaled by how large the
    /// image is shown compared to the PNG itself.
    pub fn resolve(&self, target: &Target) -> Result<(f64, f64)> {
        match target {
            Target::Point(x, y) => Ok((*x, *y)),
            Target::OnImage { image, x, y } => {
                let (r, scale) = self.image_scale(image)?;
                Ok((r.x + x * scale, r.y + y * scale))
            }
        }
    }

    /// Where an image is shown, and canvas pixels per PNG pixel.
    fn image_scale(&self, image: &str) -> Result<(Rect, f64)> {
        let e = self
            .find_live(image)
            .filter(|e| e["type"] == "image")
            .with_context(|| format!("There is no screenshot with id {image}."))?;
        let file = &self.doc["files"][str_of(e, "fileId")];
        let data = str_of(file, "dataURL")
            .split_once(',')
            .map(|(_, d)| d)
            .context("The screenshot has no image data.")?;
        let png = base64::engine::general_purpose::STANDARD
            .decode(data)
            .context("The screenshot data is not valid.")?;
        let (pw, _) = png::dimensions(&png)?;
        let r = Rect::of(e);
        Ok((r, r.w / pw as f64))
    }

    /// Annotation arrow from a note (or any element) to a spot, e.g. a field on a
    /// screenshot. Red unless another colour is given. Returns the arrow id.
    pub fn add_pointer(&mut self, from: &str, to: &Target, color: Option<Color>) -> Result<String> {
        let r = Rect::of(
            self.find_live(from)
                .with_context(|| format!("There is no element with id {from}."))?,
        );
        let (tx, ty) = self.resolve(to)?;
        let (sx, sy) = edge_towards(r, (tx, ty));
        let mut arrow = base("arrow", sx, sy, (tx - sx).abs(), (ty - sy).abs());
        arrow["points"] = json!([[0.0, 0.0], [tx - sx, ty - sy]]);
        arrow["strokeColor"] = json!(color.unwrap_or(Color::Red).stroke());
        arrow["roundness"] = json!({ "type": 2 });
        arrow["startBinding"] = json!({ "elementId": from, "focus": 0, "gap": ARROW_GAP });
        arrow["endBinding"] = Value::Null;
        arrow["startArrowhead"] = Value::Null;
        arrow["endArrowhead"] = json!("arrow");
        arrow["lastCommittedPoint"] = Value::Null;
        arrow["elbowed"] = json!(false);
        let id = str_of(&arrow, "id").to_string();
        self.push(arrow);
        let e = self.find_mut(from).expect("checked above");
        let mut bound = e["boundElements"].as_array().cloned().unwrap_or_default();
        bound.push(json!({ "type": "arrow", "id": id }));
        e["boundElements"] = json!(bound);
        bump(e);
        Ok(id)
    }

    /// Outline (no fill, no label) around part of the canvas or of a screenshot, to
    /// mark what is new or changes. `size` is in the same units as `top_left`.
    /// Red unless another colour is given. Returns its id.
    pub fn add_highlight(
        &mut self,
        top_left: &Target,
        size: (f64, f64),
        color: Option<Color>,
    ) -> Result<String> {
        let (x, y) = self.resolve(top_left)?;
        let scale = match top_left {
            Target::OnImage { image, .. } => self.image_scale(image)?.1,
            Target::Point(..) => 1.0,
        };
        let mut e = base("rectangle", x, y, size.0 * scale, size.1 * scale);
        e["strokeColor"] = json!(color.unwrap_or(Color::Red).stroke());
        e["roundness"] = json!({ "type": 3 });
        let id = str_of(&e, "id").to_string();
        self.push(e);
        Ok(id)
    }

    /// Named frame (e.g. one screen of the spec). Returns the frame id.
    pub fn add_frame(&mut self, name: &str, at: Placement, size: (f64, f64)) -> String {
        let (x, y) = self.position(at);
        let mut e = base("frame", x, y, size.0, size.1);
        e["name"] = json!(name);
        let id = str_of(&e, "id").to_string();
        self.push(e);
        id
    }

    /// PNG image embedded in the document. `width` scales it, keeping proportions.
    pub fn add_image(&mut self, png: &[u8], at: Placement, width: Option<f64>) -> Result<String> {
        let (pw, ph) = png::dimensions(png)?;
        let w = width.unwrap_or((pw as f64).min(MAX_IMAGE_WIDTH));
        let h = (w * ph as f64 / pw as f64).round();
        let (x, y) = self.position(at);

        let file_id = uuid::Uuid::new_v4().simple().to_string();
        let data = base64::engine::general_purpose::STANDARD.encode(png);
        self.doc["files"][&file_id] = json!({
            "mimeType": "image/png",
            "id": file_id,
            "dataURL": format!("data:image/png;base64,{data}"),
            "created": now_ms(),
            "lastRetrieved": now_ms(),
        });
        let mut e = base("image", x, y, w, h);
        e["strokeColor"] = json!("transparent");
        e["fileId"] = json!(file_id);
        e["status"] = json!("saved");
        e["scale"] = json!([1, 1]);
        e["crop"] = Value::Null;
        let id = str_of(&e, "id").to_string();
        self.push(e);
        Ok(id)
    }

    /// Width and height of a live element.
    pub fn size_of(&self, id: &str) -> Option<(f64, f64)> {
        self.find_live(id).map(|e| {
            let r = Rect::of(e);
            (r.w, r.h)
        })
    }

    /// Change the text of a text element, a box or arrow label, or a frame name.
    pub fn set_text(&mut self, id: &str, content: &str) -> Result<()> {
        let e = self
            .find_live(id)
            .with_context(|| format!("There is no element with id {id}."))?;
        match e["type"].as_str() {
            Some("frame") => {
                let e = self.find_mut(id).expect("found above");
                e["name"] = json!(content);
                bump(e);
            }
            Some("text") => self.replace_text(id, content),
            _ => {
                let label_id = self
                    .label_of(id)
                    .map(|l| str_of(l, "id").to_string())
                    .with_context(|| format!("Element {id} has no text to change."))?;
                self.replace_text(&label_id, content);
            }
        }
        Ok(())
    }

    /// Move an element (with its label, frame content and attached arrows).
    pub fn move_to(&mut self, id: &str, x: f64, y: f64) -> Result<()> {
        let e = self
            .find_live(id)
            .with_context(|| format!("There is no element with id {id}."))?;
        let r = Rect::of(e);
        let (dx, dy) = (x - r.x, y - r.y);
        let mut moved = vec![id.to_string()];
        moved.extend(
            self.live()
                .filter(|c| c["containerId"] == id || c["frameId"] == id)
                .map(|c| str_of(c, "id").to_string()),
        );
        for mid in &moved {
            let e = self.find_mut(mid).expect("live element");
            e["x"] = json!(e["x"].as_f64().unwrap_or(0.0) + dx);
            e["y"] = json!(e["y"].as_f64().unwrap_or(0.0) + dy);
            bump(e);
        }
        let arrows: Vec<String> = self
            .live()
            .filter(|a| a["type"] == "arrow" && moved.iter().any(|m| bound_to(a, m)))
            .map(|a| str_of(a, "id").to_string())
            .collect();
        for arrow in arrows {
            self.route_arrow(&arrow);
        }
        Ok(())
    }

    /// Delete elements (with their labels and attached arrows). Returns how many were removed.
    pub fn delete(&mut self, ids: &[String]) -> Result<usize> {
        for id in ids {
            ensure!(
                self.find_live(id).is_some(),
                "There is no element with id {id}."
            );
        }
        let mut doomed: Vec<String> = ids.to_vec();
        let arrows: Vec<String> = self
            .live()
            .filter(|a| a["type"] == "arrow" && ids.iter().any(|id| bound_to(a, id)))
            .map(|a| str_of(a, "id").to_string())
            .collect();
        doomed.extend(arrows);
        let labels: Vec<String> = self
            .live()
            .filter(|t| doomed.iter().any(|d| t["containerId"] == d.as_str()))
            .map(|t| str_of(t, "id").to_string())
            .collect();
        doomed.extend(labels);
        doomed.sort();
        doomed.dedup();

        for e in self.elements_mut() {
            let id = str_of(e, "id").to_string();
            if doomed.contains(&id) {
                e["isDeleted"] = json!(true);
                bump(e);
            } else {
                let mut changed = false;
                if let Some(bound) = e["boundElements"].as_array() {
                    let kept: Vec<Value> = bound
                        .iter()
                        .filter(|b| !doomed.iter().any(|d| b["id"] == d.as_str()))
                        .cloned()
                        .collect();
                    if kept.len() != bound.len() {
                        e["boundElements"] = json!(kept);
                        changed = true;
                    }
                }
                // Children of a deleted frame stay, outside any frame.
                if e["frameId"]
                    .as_str()
                    .is_some_and(|f| doomed.iter().any(|d| d == f))
                {
                    e["frameId"] = Value::Null;
                    changed = true;
                }
                if changed {
                    bump(e);
                }
            }
        }
        Ok(doomed.len())
    }

    // ---- internals ----

    fn elements(&self) -> &Vec<Value> {
        self.doc["elements"].as_array().expect("checked in parse")
    }

    fn elements_mut(&mut self) -> &mut Vec<Value> {
        self.doc["elements"]
            .as_array_mut()
            .expect("checked in parse")
    }

    fn live(&self) -> impl Iterator<Item = &Value> {
        self.elements().iter().filter(|e| e["isDeleted"] != true)
    }

    fn find(&self, id: &str) -> Option<&Value> {
        self.elements().iter().find(|e| e["id"] == id)
    }

    fn find_live(&self, id: &str) -> Option<&Value> {
        self.live().find(|e| e["id"] == id)
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut Value> {
        self.elements_mut().iter_mut().find(|e| e["id"] == id)
    }

    fn label_of(&self, container: &str) -> Option<&Value> {
        self.live()
            .find(|t| t["type"] == "text" && t["containerId"] == container)
    }

    fn push(&mut self, e: Value) {
        self.elements_mut().push(e);
    }

    /// Requested position, with missing coordinates filled in below the content.
    fn position(&self, at: Placement) -> (f64, f64) {
        let placed: Vec<Rect> = self.live().filter(|e| !is_label(e)).map(Rect::of).collect();
        let below = placed
            .iter()
            .map(|r| r.y + r.h)
            .fold(f64::NEG_INFINITY, f64::max);
        let left = placed.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
        let (default_x, default_y) = if placed.is_empty() {
            (0.0, 0.0)
        } else {
            (left, below + GAP)
        };
        (at.x.unwrap_or(default_x), at.y.unwrap_or(default_y))
    }

    fn replace_text(&mut self, text_id: &str, content: &str) {
        let container = self
            .find(text_id)
            .and_then(|t| t["containerId"].as_str().map(String::from));
        let container_rect = container
            .as_deref()
            .and_then(|c| self.find(c))
            .map(Rect::of);
        let e = self.find_mut(text_id).expect("caller checked");
        let size = e["fontSize"].as_f64().unwrap_or(text::FONT_SIZE);
        let wrapped = match container_rect {
            Some(r) if container.is_some() => text::wrap(content, r.w - 2.0 * BOX_PADDING, size),
            _ => content.to_string(),
        };
        let (w, h) = text::measure(&wrapped, size);
        e["text"] = json!(wrapped);
        e["originalText"] = json!(content);
        e["width"] = json!(w);
        e["height"] = json!(h);
        bump(e);
        let Some(mut r) = container_rect else { return };
        // A longer label makes its box taller rather than spilling out of it.
        let is_box = container
            .as_deref()
            .and_then(|c| self.find(c))
            .is_some_and(|c| c["type"] != "arrow");
        if is_box && r.h < h + 2.0 * LABEL_PADDING_Y {
            r.h = h + 2.0 * LABEL_PADDING_Y;
            let c = self
                .find_mut(container.as_deref().expect("checked"))
                .expect("container exists");
            c["height"] = json!(r.h);
            bump(c);
        }
        let (cx, cy) = r.center();
        let e = self.find_mut(text_id).expect("caller checked");
        e["x"] = json!(cx - w / 2.0);
        e["y"] = json!(cy - h / 2.0);
    }

    /// Straight arrow between the facing edges of its two bound elements.
    fn route_arrow(&mut self, arrow_id: &str) {
        let Some(arrow) = self.find(arrow_id) else {
            return;
        };
        let end = |key: &str| {
            arrow[key]["elementId"]
                .as_str()
                .and_then(|id| self.find(id))
                .map(Rect::of)
        };
        let (Some(a), Some(b)) = (end("startBinding"), end("endBinding")) else {
            return;
        };
        let ((ax, ay), (bx, by)) = (a.center(), b.center());
        let (start, finish) = if (by - ay).abs() >= (bx - ax).abs() {
            let s = (by - ay).signum();
            (
                (ax, ay + s * (a.h / 2.0 + ARROW_GAP)),
                (bx, by - s * (b.h / 2.0 + ARROW_GAP)),
            )
        } else {
            let s = (bx - ax).signum();
            (
                (ax + s * (a.w / 2.0 + ARROW_GAP), ay),
                (bx - s * (b.w / 2.0 + ARROW_GAP), by),
            )
        };
        let (dx, dy) = (finish.0 - start.0, finish.1 - start.1);
        let label = self.label_of(arrow_id).map(|l| str_of(l, "id").to_string());
        let e = self.find_mut(arrow_id).expect("found above");
        e["x"] = json!(start.0);
        e["y"] = json!(start.1);
        e["points"] = json!([[0.0, 0.0], [dx, dy]]);
        e["width"] = json!(dx.abs());
        e["height"] = json!(dy.abs());
        bump(e);
        if let Some(label) = label {
            let l = self.find_mut(&label).expect("live label");
            let (lw, lh) = (
                l["width"].as_f64().unwrap_or(0.0),
                l["height"].as_f64().unwrap_or(0.0),
            );
            l["x"] = json!(start.0 + dx / 2.0 - lw / 2.0);
            l["y"] = json!(start.1 + dy / 2.0 - lh / 2.0);
            bump(l);
        }
    }
}

/// Fields every Excalidraw element needs, with the editor's defaults.
fn base(kind: &str, x: f64, y: f64, width: f64, height: f64) -> Value {
    json!({
        "id": random_id(),
        "type": kind,
        "x": x,
        "y": y,
        "width": width,
        "height": height,
        "angle": 0,
        "strokeColor": INK,
        "backgroundColor": "transparent",
        "fillStyle": "solid",
        "strokeWidth": 2,
        "strokeStyle": "solid",
        "roughness": 1,
        "opacity": 100,
        "groupIds": [],
        "frameId": null,
        "roundness": null,
        "seed": random_u32(),
        "version": 1,
        "versionNonce": random_u32(),
        "isDeleted": false,
        "boundElements": null,
        "updated": now_ms(),
        "link": null,
        "locked": false,
    })
}

/// Text centred in `container` (a box or an arrow's bounding box).
fn label_element(
    wrapped: &str,
    original: &str,
    container: &str,
    r: Rect,
    frame: Option<&str>,
) -> Value {
    let (w, h) = text::measure(wrapped, text::FONT_SIZE);
    let (cx, cy) = r.center();
    let mut e = base("text", cx - w / 2.0, cy - h / 2.0, w, h);
    set_text_fields(
        &mut e,
        wrapped,
        original,
        text::FONT_SIZE,
        "center",
        "middle",
        Some(container),
    );
    e["frameId"] = json!(frame);
    e
}

fn set_text_fields(
    e: &mut Value,
    wrapped: &str,
    original: &str,
    font_size: f64,
    align: &str,
    valign: &str,
    container: Option<&str>,
) {
    let fields: Map<String, Value> = serde_json::from_value(json!({
        "text": wrapped,
        "originalText": original,
        "fontSize": font_size,
        "fontFamily": 5,
        "textAlign": align,
        "verticalAlign": valign,
        "containerId": container,
        "autoResize": true,
        "lineHeight": text::LINE_HEIGHT,
    }))
    .expect("object literal");
    e.as_object_mut()
        .expect("element is an object")
        .extend(fields);
}

/// Where the line from the centre of `r` towards `target` leaves `r`, plus the
/// usual arrow gap, so a pointer starts at the edge of its note.
fn edge_towards(r: Rect, target: (f64, f64)) -> (f64, f64) {
    let (cx, cy) = r.center();
    let (dx, dy) = (target.0 - cx, target.1 - cy);
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let half_w = r.w / 2.0 + ARROW_GAP;
    let half_h = r.h / 2.0 + ARROW_GAP;
    let t = (half_w / dx.abs()).min(half_h / dy.abs()).min(1.0);
    (cx + dx * t, cy + dy * t)
}

fn is_label(e: &Value) -> bool {
    e["type"] == "text" && e["containerId"].is_string()
}

fn bound_to(arrow: &Value, id: &str) -> bool {
    arrow["startBinding"]["elementId"] == id || arrow["endBinding"]["elementId"] == id
}

fn bump(e: &mut Value) {
    e["version"] = json!(e["version"].as_u64().unwrap_or(0) + 1);
    e["versionNonce"] = json!(random_u32());
    e["updated"] = json!(now_ms());
}

fn str_of<'a>(e: &'a Value, key: &str) -> &'a str {
    e[key].as_str().unwrap_or_default()
}

fn random_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..20].to_string()
}

fn random_u32() -> u32 {
    (uuid::Uuid::new_v4().as_u128() & 0x7fff_ffff) as u32
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
