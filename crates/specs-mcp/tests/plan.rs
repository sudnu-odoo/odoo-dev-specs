//! The spec builder: a plan in, a laid-out diagram out, with nothing overlapping.

use std::collections::HashMap;

use excalidraw::Scene;
use serde_json::{json, Value};
use specs_mcp::browser::Element;
use specs_mcp::plan::{build, validate, ShotData, SpecPlan, TAG};

const EMPTY: &str = r##"{"type":"excalidraw","version":2,"source":"x","elements":[],"appState":{"viewBackgroundColor":"#ffffff"},"files":{}}"##;

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    bytes.extend(b"IHDR");
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0]);
    bytes
}

fn el(kind: &str, name: &str, text: &str, x: f64, y: f64, w: f64, h: f64) -> Element {
    serde_json::from_value(
        json!({"kind": kind, "name": name, "text": text, "x": x, "y": y, "w": w, "h": h}),
    )
    .unwrap()
}

/// A quotation form 1440x900: Expiration label and field, and a Confirm button.
fn quotation() -> ShotData {
    ShotData {
        path: "screenshots/quotation.png".into(),
        png: png(1440, 900),
        map: vec![
            el(
                "label",
                "validity_date",
                "Expiration",
                100.0,
                300.0,
                120.0,
                24.0,
            ),
            el(
                "field",
                "validity_date",
                "12/31/2026",
                300.0,
                300.0,
                200.0,
                24.0,
            ),
            el(
                "button",
                "action_confirm",
                "Confirm",
                40.0,
                120.0,
                90.0,
                30.0,
            ),
        ],
    }
}

fn shots(names: &[&str]) -> HashMap<String, ShotData> {
    names.iter().map(|n| (n.to_string(), quotation())).collect()
}

fn plan(v: Value) -> SpecPlan {
    serde_json::from_value(v).unwrap()
}

fn sample() -> SpecPlan {
    plan(json!({
        "title": "Delivery deadline on quotations",
        "need": "Sales want to promise a delivery date on each quotation.",
        "context": ["Odoo Online 19.0", "Customers ask for a date before they sign."],
        "version": "19.0",
        "modules": ["sale_management"],
        "sections": [
            {"title": "Sales > Orders > Quotation",
             "text": ["1. The salesperson fills the quotation", "2. Confirms it"],
             "screens": [{"name": "today", "page": "/odoo/sales/3", "model": "sale.order", "caption": "CURRENT",
                          "notes": [{"lines": ["Confirm is allowed", "without a date"], "anchor": "button:action_confirm"}]}]},
            {"title": "Quotation form",
             "screens": [{"name": "expected", "page": "/odoo/sales/3", "model": "sale.order",
                          "new_fields": [{"near": "validity_date", "label": "Delivery Deadline", "value": "12/20/2026",
                                          "technical": "x_delivery_deadline", "type": "date"}],
                          "notes": [{"lines": ["Blocked if the", "deadline is empty"], "anchor": "button:action_confirm"},
                                    {"lines": ["Copied to the delivery"]}]}]}
        ],
        "links": [{"from": "today", "to": "expected", "label": "becomes"}],
        "tests": ["Salesperson: confirms without a deadline -> blocked", "Manager: sets a deadline -> confirmed"]
    }))
}

fn live(scene: &Scene) -> Vec<Value> {
    let doc: Value = serde_json::from_str(&scene.to_json()).unwrap();
    doc["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isDeleted"] != true)
        .cloned()
        .collect()
}

fn texts(scene: &Scene) -> Vec<String> {
    live(scene)
        .iter()
        .filter(|e| e["type"] == "text")
        .map(|e| e["text"].as_str().unwrap().to_string())
        .collect()
}

fn find_text<'a>(all: &'a [Value], starts: &str) -> &'a Value {
    all.iter()
        .find(|e| e["type"] == "text" && e["text"].as_str().unwrap().starts_with(starts))
        .unwrap_or_else(|| panic!("no text starting with {starts}"))
}

#[test]
fn builds_the_whole_spec_in_one_go_without_overlaps() {
    let mut scene = Scene::parse(EMPTY).unwrap();
    let report = build(&mut scene, &sample(), &shots(&["today", "expected"])).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let t = texts(&scene);
    for want in [
        "Delivery deadline on quotations",
        "1. Sales > Orders > Quotation",
        "2. Quotation form",
        "Model: sale.order",
        "CURRENT",
        "Test cases",
    ] {
        assert!(t.iter().any(|x| x == want), "missing {want} in {t:?}");
    }
    assert!(t
        .iter()
        .any(|x| x.contains("[ ] 1. Salesperson") && x.contains("[ ] 2. Manager")));
    assert!(t
        .iter()
        .any(|x| x.starts_with("New field: Delivery Deadline (x_delivery_deadline, date)")));
    // Nothing is written over the screenshot: a field not prototyped is pointed at.
    assert!(!t
        .iter()
        .any(|x| x == "Delivery Deadline" || x == "12/20/2026"));
    // Every readable text stands alone.
    for e in live(&scene)
        .iter()
        .filter(|e| e["type"] == "text" && e["containerId"].is_null())
    {
        let id = e["id"].as_str().unwrap();
        assert!(
            scene.overlapping(id).is_empty(),
            "{} overlaps {:?}",
            e["text"],
            scene.overlapping(id)
        );
    }
}

#[test]
fn each_requirement_gets_a_plain_frame_stacked_top_to_bottom() {
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &sample(), &shots(&["today", "expected"])).unwrap();
    let all = live(&scene);
    let first = find_text(&all, "1. ")["y"].as_f64().unwrap();
    let second = find_text(&all, "2. ")["y"].as_f64().unwrap();
    let tests = find_text(&all, "Test cases")["y"].as_f64().unwrap();
    assert!(first < second && second < tests);
    // A frame is an outline only: no fill, no tint.
    let frames: Vec<&Value> = all
        .iter()
        .filter(|e| e["type"] == "rectangle" && e["customData"]["frame"] == true)
        .collect();
    assert_eq!(frames.len(), 2);
    for f in &frames {
        assert_eq!(f["backgroundColor"], "transparent");
        assert_ne!(f["strokeColor"], "#e03131");
    }
    let p = frames
        .iter()
        .find(|p| p["y"].as_f64().unwrap() < first)
        .unwrap();
    assert!(p["y"].as_f64().unwrap() + p["height"].as_f64().unwrap() < second);
}

#[test]
fn notes_sit_right_of_the_screenshot_level_with_what_they_point_at() {
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &sample(), &shots(&["today", "expected"])).unwrap();
    let all = live(&scene);
    let image = all.iter().find(|e| e["type"] == "image").unwrap();
    let (ix, iy, iw) = (
        image["x"].as_f64().unwrap(),
        image["y"].as_f64().unwrap(),
        image["width"].as_f64().unwrap(),
    );
    let note = find_text(&all, "Confirm is allowed");
    assert!(
        note["x"].as_f64().unwrap() >= ix + iw,
        "note right of the screenshot"
    );
    // Confirm is at y 120..150 of 900 px, shown 900 wide: scale 0.625.
    let button_mid = iy + 135.0 * 900.0 / 1440.0;
    assert!((note["y"].as_f64().unwrap() - button_mid).abs() < 40.0);
    assert_eq!(image["customData"]["source"], "screenshots/quotation.png");
    // Outline + pointer for an anchored note.
    assert!(all
        .iter()
        .any(|e| e["type"] == "rectangle" && e["strokeColor"] == "#e03131" && e["opacity"] == 100));
}

#[test]
fn links_are_red_dashed_arrows_between_screens() {
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &sample(), &shots(&["today", "expected"])).unwrap();
    let all = live(&scene);
    let dashed: Vec<&Value> = all
        .iter()
        .filter(|e| e["type"] == "arrow" && e["strokeStyle"] == "dashed")
        .collect();
    assert_eq!(dashed.len(), 1);
    assert_eq!(dashed[0]["strokeColor"], "#e03131");
}

#[test]
fn horizontal_requirements_put_screens_in_a_row() {
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "Current vs expected", "horizontal": true,
            "screens": [{"name": "a", "page": "/odoo", "caption": "CURRENT"},
                        {"name": "b", "page": "/odoo", "caption": "EXPECTED"}]}]
    }));
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &p, &shots(&["a", "b"])).unwrap();
    let images: Vec<Value> = live(&scene)
        .into_iter()
        .filter(|e| e["type"] == "image")
        .collect();
    assert_eq!(images[0]["y"], images[1]["y"]);
    assert!(images[1]["x"].as_f64().unwrap() > images[0]["x"].as_f64().unwrap() + 700.0);
}

#[test]
fn a_new_plan_replaces_the_last_build_but_keeps_hand_made_elements() {
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &sample(), &shots(&["today", "expected"])).unwrap();
    let mine = scene.add_text("my own note", Default::default(), None, None);
    let before = scene.tagged(TAG).len();
    build(&mut scene, &sample(), &shots(&["today", "expected"])).unwrap();
    assert_eq!(scene.tagged(TAG).len(), before, "no duplicates");
    assert!(texts(&scene).contains(&"my own note".to_string()));
    assert!(!scene.tagged(TAG).contains(&mine));
}

#[test]
fn an_unknown_anchor_is_a_warning_and_the_note_is_still_written() {
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "S", "screens": [{"name": "a", "page": "/odoo",
            "notes": [{"lines": ["about nothing"], "anchor": "button:does_not_exist"}]}]}]
    }));
    let mut scene = Scene::parse(EMPTY).unwrap();
    let report = build(&mut scene, &p, &shots(&["a"])).unwrap();
    assert_eq!(report.warnings.len(), 1);
    assert!(
        report.warnings[0].contains("does_not_exist"),
        "{:?}",
        report.warnings
    );
    assert!(texts(&scene).contains(&"about nothing".to_string()));
}

#[test]
fn plan_mistakes_are_explained() {
    let dup = plan(json!({"title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "S", "screens": [{"name": "a", "page": "/x"}, {"name": "a", "page": "/y"}]}]}));
    assert!(validate(&dup).unwrap_err().to_string().contains("unique"));
    let link = plan(json!({"title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "S", "screens": [{"name": "a", "page": "/x"}]}],
        "links": [{"from": "a", "to": "zz"}]}));
    assert!(validate(&link).unwrap_err().to_string().contains("zz"));
    let long = plan(json!({"title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "S", "screens": [{"name": "a", "page": "/x",
            "notes": [{"lines": ["one", "two", "three", "four"]}]}]}]}));
    assert!(validate(&long).unwrap_err().to_string().contains("3 lines"));
    let code = plan(json!({"title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "S", "text": ["Then record.action_cancel()"]}]}));
    assert!(validate(&code)
        .unwrap_err()
        .to_string()
        .contains("business words"));
}

#[test]
fn a_prototyped_field_is_outlined_where_it_really_is() {
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "prototype": {"fields": [{"model": "sale.order", "name": "x_delivery_deadline", "label": "Delivery Deadline",
                                  "type": "date", "after": "validity_date"}]},
        "sections": [{"title": "S", "screens": [{"name": "a", "page": "/odoo/sales/3",
            "new_fields": [{"near": "validity_date", "label": "Delivery Deadline", "value": "12/20/2026",
                            "technical": "x_delivery_deadline", "type": "date"}]}]}]
    }));
    let mut shot = quotation();
    shot.map.push(el(
        "label",
        "x_delivery_deadline",
        "Delivery Deadline",
        100.0,
        340.0,
        150.0,
        24.0,
    ));
    shot.map.push(el(
        "field",
        "x_delivery_deadline",
        "",
        300.0,
        340.0,
        200.0,
        24.0,
    ));
    let mut scene = Scene::parse(EMPTY).unwrap();
    let report = build(&mut scene, &p, &HashMap::from([("a".to_string(), shot)])).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let t = texts(&scene);
    assert!(
        !t.contains(&"Delivery Deadline".to_string()),
        "no fake label: the real one is on the screen"
    );
    assert!(t
        .iter()
        .any(|x| x.starts_with("New field: Delivery Deadline (x_delivery_deadline")));
    let all = live(&scene);
    assert!(all
        .iter()
        .any(|e| e["type"] == "rectangle" && e["strokeColor"] == "#e03131" && e["opacity"] == 100));
}

#[test]
fn a_prototype_page_must_name_a_new_model() {
    let p = plan(json!({"title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "S", "screens": [{"name": "a", "page": "model:x_matrix"}]}]}));
    assert!(validate(&p).unwrap_err().to_string().contains("x_matrix"));
}

#[test]
fn notes_follow_the_order_of_what_they_point_at_so_arrows_do_not_cross() {
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &sample(), &shots(&["today", "expected"])).unwrap();
    let all = live(&scene);
    // Confirm (y 120) is above Expiration (y 300) on the screenshot.
    let button = find_text(&all, "Blocked if the")["y"].as_f64().unwrap();
    let field = find_text(&all, "New field: Delivery Deadline")["y"]
        .as_f64()
        .unwrap();
    assert!(
        button < field,
        "button note {button} should be above field note {field}"
    );
}

#[test]
fn a_single_requirement_needs_no_frame() {
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "Revise a quotation", "text": ["1. Click", "2. A copy opens"],
            "screens": [{"name": "a", "page": "/odoo/sales/3"}, {"name": "b", "page": "/odoo/sales"}]}]
    }));
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &p, &shots(&["a", "b"])).unwrap();
    assert!(!live(&scene)
        .iter()
        .any(|e| e["customData"]["frame"] == true));
}

#[test]
fn a_long_note_pushes_the_next_screen_of_a_row_to_the_right() {
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "Both PDFs", "horizontal": true,
            "screens": [{"name": "a", "page": "/odoo", "notes": [{"lines": [
                "A rather long note line that is much wider than the usual notes column is"]}]},
                        {"name": "b", "page": "/odoo"}]}]
    }));
    let mut scene = Scene::parse(EMPTY).unwrap();
    let report = build(&mut scene, &p, &shots(&["a", "b"])).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let all = live(&scene);
    let note = find_text(&all, "A rather long");
    let note_right = note["x"].as_f64().unwrap() + note["width"].as_f64().unwrap();
    let second = all.iter().filter(|e| e["type"] == "image").nth(1).unwrap();
    assert!(second["x"].as_f64().unwrap() > note_right);
}

#[test]
fn a_section_can_show_a_mock_up_table_of_the_expected_result() {
    let p = plan(json!({
        "title": "Stock card", "need": "N", "version": "19.0",
        "sections": [{"title": "Expected report", "text": ["Grouped by product"],
            "table": {"caption": "Stock card (expected)",
                "columns": [
                    {"name": "Product", "source": "product_id"},
                    {"name": "Opening", "source": "stock at the start date"},
                    {"name": "Closing", "source": "Opening + In - Out"}],
                "rows": [["Desk", "10", "12"], ["Chair", "4", "1"]]},
            "screens": [{"name": "a", "page": "/odoo/inventory"}]}]
    }));
    let mut scene = Scene::parse(EMPTY).unwrap();
    let report = build(&mut scene, &p, &shots(&["a"])).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let all = live(&scene);
    for t in [
        "Stock card (expected)",
        "Product",
        "Opening + In - Out",
        "Desk",
        "12",
    ] {
        find_text(&all, t);
    }
    // The table sits above the screen, and its cells are drawn as a grid.
    let header = find_text(&all, "Product");
    let image = all.iter().find(|e| e["type"] == "image").unwrap();
    assert!(header["y"].as_f64() < image["y"].as_f64());
    let cells = all.iter().filter(|e| e["type"] == "rectangle").count();
    assert!(cells >= 9, "{cells} rectangles");
}

#[test]
fn a_table_row_must_have_one_value_per_column() {
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "R", "table": {"columns": [{"name": "A"}, {"name": "B"}], "rows": [["only one"]]}}]
    }));
    assert!(validate(&p).unwrap_err().to_string().contains("column"));
}

#[test]
fn new_fields_left_out_of_the_prototype_are_prototyped_for_real() {
    let mut p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "Limit", "screens": [{"name": "team", "page": "/odoo/action-sales_team.crm_team_action_config/1",
            "model": "crm.team",
            "new_fields": [
                {"near": "user_id", "label": "Max. Discount (%)", "technical": "max_discount", "type": "float"},
                {"near": "user_id", "label": "Approver", "technical": "x_approver_id", "type": "many2one (res.users)"},
                {"near": "user_id", "label": "Lines", "technical": "line_ids", "type": "one2many"}]}]}]
    }));
    specs_mcp::plan::auto_prototype(&mut p);
    let fields = &p.prototype.as_ref().unwrap().fields;
    let names: Vec<_> = fields
        .iter()
        .map(|f| {
            (
                f.name.as_str(),
                f.field_type.as_str(),
                f.relation.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("x_max_discount", "float", None),
            ("x_approver_id", "many2one", Some("res.users"))
        ],
        "a one2many cannot be made as data: it keeps its note"
    );
    assert!(fields
        .iter()
        .all(|f| f.model == "crm.team" && f.after.as_deref() == Some("user_id")));
}

#[test]
fn a_new_field_that_could_not_be_prototyped_writes_nothing_on_the_screenshot() {
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "R", "screens": [{"name": "a", "page": "/odoo/sales/1",
            "new_fields": [{"near": "validity_date", "label": "Deadline", "value": "12/20", "technical": "x_deadline", "type": "date"}]}]}]
    }));
    let mut scene = Scene::parse(EMPTY).unwrap();
    build(&mut scene, &p, &shots(&["a"])).unwrap();
    let all = texts(&scene);
    assert!(
        !all.iter().any(|t| t == "Deadline" || t == "12/20"),
        "{all:?}"
    );
    assert!(
        all.iter().any(|t| t.starts_with("New field: Deadline")),
        "{all:?}"
    );
}

#[test]
fn crowded_notes_get_numbers_instead_of_crossing_arrows() {
    // Three things on the same line (e.g. header buttons): their notes cannot all be
    // level with them, so each gets a number on the screenshot and in its note.
    let mut shot = quotation();
    shot.map
        .push(el("button", "action_a", "Ask", 140.0, 120.0, 60.0, 30.0));
    shot.map
        .push(el("button", "action_b", "Reject", 210.0, 120.0, 60.0, 30.0));
    let p = plan(json!({
        "title": "T", "need": "N", "version": "19.0",
        "sections": [{"title": "R", "screens": [{"name": "a", "page": "/odoo/sales/1", "notes": [
            {"lines": ["Confirm hidden while waiting"], "anchor": "action_confirm"},
            {"lines": ["New: Ask Approval"], "anchor": "action_a"},
            {"lines": ["New: Reject"], "anchor": "action_b"}]}]}]
    }));
    let mut scene = Scene::parse(EMPTY).unwrap();
    let report = build(
        &mut scene,
        &p,
        &[("a".to_string(), shot)].into_iter().collect(),
    )
    .unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let all = texts(&scene);
    assert!(all.iter().any(|t| t.starts_with("1. ")), "{all:?}");
    assert!(
        all.iter().any(|t| t.starts_with("3. New: Reject")),
        "{all:?}"
    );
    let arrows = live(&scene).iter().filter(|e| e["type"] == "arrow").count();
    assert!(
        arrows <= 1,
        "{arrows} arrows: crowded notes must not get arrows"
    );
}
