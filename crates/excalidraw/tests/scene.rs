use excalidraw::{Color, Placement, Scene, Target};
use serde_json::Value;

const EMPTY: &str = r##"{"type":"excalidraw","version":2,"source":"x","elements":[],"appState":{"viewBackgroundColor":"#ffffff"},"files":{}}"##;

fn empty() -> Scene {
    Scene::parse(EMPTY).unwrap()
}

fn doc(scene: &Scene) -> Value {
    serde_json::from_str(&scene.to_json()).unwrap()
}

fn element(scene: &Scene, id: &str) -> Value {
    doc(scene)["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("no element {id}"))
}

fn at(x: f64, y: f64) -> Placement {
    Placement {
        x: Some(x),
        y: Some(y),
    }
}

/// Smallest valid PNG header with the given size (only IHDR is read).
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    bytes.extend(b"IHDR");
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0]);
    bytes
}

#[test]
fn rejects_documents_that_are_not_excalidraw() {
    assert!(Scene::parse("nope").is_err());
    assert!(Scene::parse(r#"{"type":"other","elements":[]}"#).is_err());
    assert!(Scene::parse(r#"{"type":"excalidraw"}"#).is_err());
}

#[test]
fn keeps_unknown_fields_and_app_state() {
    let json = r##"{"type":"excalidraw","version":2,"source":"x","elements":[{"id":"a","type":"rectangle","x":1,"y":2,"width":3,"height":4,"version":7,"isDeleted":false,"custom":"kept"}],"appState":{"viewBackgroundColor":"#123456"},"files":{}}"##;
    let out = doc(&Scene::parse(json).unwrap());
    assert_eq!(out["elements"][0]["custom"], "kept");
    assert_eq!(out["appState"]["viewBackgroundColor"], "#123456");
}

#[test]
fn colors_by_name() {
    assert_eq!(Color::parse("Blue"), Some(Color::Blue));
    assert_eq!(Color::parse(" green "), Some(Color::Green));
    assert_eq!(Color::parse("chartreuse"), None);
}

#[test]
fn box_is_a_rectangle_with_a_bound_centred_label() {
    let mut s = empty();
    let id = s
        .add_box(
            "Sales order",
            at(10.0, 20.0),
            Some((200.0, 80.0)),
            Some(Color::Blue),
            None,
        )
        .unwrap();
    let rect = element(&s, &id);
    assert_eq!(rect["type"], "rectangle");
    assert_eq!(
        (rect["x"].as_f64(), rect["y"].as_f64()),
        (Some(10.0), Some(20.0))
    );
    assert_eq!(
        (rect["width"].as_f64(), rect["height"].as_f64()),
        (Some(200.0), Some(80.0))
    );
    assert_eq!(rect["backgroundColor"], "#a5d8ff");
    assert_eq!(rect["version"], 1);
    assert_eq!(rect["isDeleted"], false);

    let text_id = rect["boundElements"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(rect["boundElements"][0]["type"], "text");
    let label = element(&s, &text_id);
    assert_eq!(label["type"], "text");
    assert_eq!(label["text"], "Sales order");
    assert_eq!(label["containerId"], id.as_str());
    assert_eq!(label["textAlign"], "center");
    assert_eq!(label["verticalAlign"], "middle");
}

#[test]
fn every_element_has_the_fields_excalidraw_needs() {
    let mut s = empty();
    let a = s
        .add_box("A", Placement::default(), None, None, None)
        .unwrap();
    let b = s
        .add_box("B", Placement::default(), None, None, None)
        .unwrap();
    s.add_arrow(&a, &b, Some("then")).unwrap();
    s.add_text("Title", Placement::default(), None, None);
    s.add_frame("Screen", Placement::default(), (400.0, 300.0));
    s.add_image(&png(10, 10), Placement::default(), None)
        .unwrap();
    for e in doc(&s)["elements"].as_array().unwrap() {
        for field in [
            "id",
            "type",
            "x",
            "y",
            "width",
            "height",
            "angle",
            "strokeColor",
            "backgroundColor",
            "fillStyle",
            "strokeWidth",
            "strokeStyle",
            "roughness",
            "opacity",
            "groupIds",
            "seed",
            "version",
            "versionNonce",
            "isDeleted",
            "updated",
            "locked",
        ] {
            assert!(!e[field].is_null(), "{} lacks {field}", e["type"]);
        }
        assert!(
            e.get("frameId").is_some()
                && e.get("boundElements").is_some()
                && e.get("link").is_some()
        );
    }
}

#[test]
fn new_elements_without_position_go_below_everything() {
    let mut s = empty();
    let first = s
        .add_box("A", at(100.0, 50.0), Some((200.0, 80.0)), None, None)
        .unwrap();
    let second = s
        .add_box("B", Placement::default(), None, None, None)
        .unwrap();
    let a = element(&s, &first);
    let b = element(&s, &second);
    assert_eq!(b["x"], a["x"]);
    assert!(b["y"].as_f64().unwrap() >= 50.0 + 80.0 + 20.0, "{}", b["y"]);

    let mut fresh = empty();
    let id = fresh.add_text("T", Placement::default(), None, None);
    assert_eq!(
        (
            element(&fresh, &id)["x"].as_f64(),
            element(&fresh, &id)["y"].as_f64()
        ),
        (Some(0.0), Some(0.0))
    );
}

#[test]
fn box_grows_to_fit_long_labels() {
    let mut s = empty();
    let id = s
        .add_box(&"word ".repeat(30), Placement::default(), None, None, None)
        .unwrap();
    let rect = element(&s, &id);
    assert!(rect["width"].as_f64().unwrap() <= 400.0);
    assert!(
        rect["height"].as_f64().unwrap() > 80.0,
        "multi-line label needs a taller box"
    );
}

#[test]
fn arrow_is_bound_to_both_ends_and_registered_on_them() {
    let mut s = empty();
    let a = s
        .add_box("A", at(0.0, 0.0), Some((100.0, 50.0)), None, None)
        .unwrap();
    let b = s
        .add_box("B", at(0.0, 200.0), Some((100.0, 50.0)), None, None)
        .unwrap();
    let arrow_id = s.add_arrow(&a, &b, Some("creates")).unwrap();
    let arrow = element(&s, &arrow_id);
    assert_eq!(arrow["type"], "arrow");
    assert_eq!(arrow["startBinding"]["elementId"], a.as_str());
    assert_eq!(arrow["endBinding"]["elementId"], b.as_str());
    assert_eq!(arrow["endArrowhead"], "arrow");
    let points = arrow["points"].as_array().unwrap();
    assert_eq!(points[0], serde_json::json!([0.0, 0.0]));
    assert!(
        points[1][1].as_f64().unwrap() > 0.0,
        "points down towards B"
    );

    for end in [&a, &b] {
        let bound = element(&s, end)["boundElements"].clone();
        assert!(bound
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == arrow_id.as_str()));
    }
    let labels: Vec<_> = s
        .summary()
        .into_iter()
        .filter(|e| e.id == arrow_id)
        .collect();
    assert_eq!(labels[0].text.as_deref(), Some("creates"));
    assert!(s.add_arrow(&a, "missing", None).is_err());
}

#[test]
fn boxes_can_be_placed_in_a_frame() {
    let mut s = empty();
    let frame = s.add_frame("Sales order form", at(0.0, 0.0), (800.0, 600.0));
    let id = s
        .add_box("Field", at(20.0, 40.0), None, None, Some(&frame))
        .unwrap();
    assert_eq!(element(&s, &id)["frameId"], frame.as_str());
    assert_eq!(element(&s, &frame)["name"], "Sales order form");
    assert!(s
        .add_box("x", Placement::default(), None, None, Some("missing"))
        .is_err());
}

#[test]
fn images_are_embedded_with_their_proportions() {
    let mut s = empty();
    let id = s
        .add_image(&png(1200, 600), at(0.0, 0.0), Some(600.0))
        .unwrap();
    let img = element(&s, &id);
    assert_eq!(img["type"], "image");
    assert_eq!(
        (img["width"].as_f64(), img["height"].as_f64()),
        (Some(600.0), Some(300.0))
    );
    let file_id = img["fileId"].as_str().unwrap();
    let file = &doc(&s)["files"][file_id];
    assert_eq!(file["mimeType"], "image/png");
    assert!(file["dataURL"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,iVBORw0KGgo"));
    assert!(s
        .add_image(b"not a png", Placement::default(), None)
        .is_err());
}

#[test]
fn summary_lists_live_elements_with_box_labels() {
    let mut s = empty();
    let frame = s.add_frame("Screen", at(0.0, 0.0), (500.0, 400.0));
    let b = s
        .add_box(
            "Customer",
            at(10.0, 10.0),
            Some((120.0, 60.0)),
            None,
            Some(&frame),
        )
        .unwrap();
    s.add_text("Note", at(0.0, 500.0), None, None);
    let summary = s.summary();
    assert_eq!(
        summary.len(),
        3,
        "the box label is folded into the box: {summary:?}"
    );
    let boxed = summary.iter().find(|e| e.id == b).unwrap();
    assert_eq!(
        (boxed.kind.as_str(), boxed.text.as_deref()),
        ("rectangle", Some("Customer"))
    );
    assert_eq!(boxed.frame.as_deref(), Some(frame.as_str()));
}

#[test]
fn set_text_and_move_bump_versions() {
    let mut s = empty();
    let b = s.add_box("Old", at(0.0, 0.0), None, None, None).unwrap();
    s.set_text(&b, "New label").unwrap();
    let label_id = element(&s, &b)["boundElements"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(element(&s, &label_id)["text"], "New label");
    assert_eq!(element(&s, &label_id)["version"], 2);

    s.move_to(&b, 300.0, 400.0).unwrap();
    let rect = element(&s, &b);
    assert_eq!(
        (rect["x"].as_f64(), rect["y"].as_f64()),
        (Some(300.0), Some(400.0))
    );
    assert_eq!(rect["version"], 2);
    let label = element(&s, &label_id);
    assert!(
        label["x"].as_f64().unwrap() >= 300.0,
        "label moves with its box"
    );
    assert!(s.set_text("missing", "x").is_err());
}

#[test]
fn delete_marks_elements_with_labels_and_arrows() {
    let mut s = empty();
    let a = s
        .add_box("A", Placement::default(), None, None, None)
        .unwrap();
    let b = s
        .add_box("B", Placement::default(), None, None, None)
        .unwrap();
    let arrow = s.add_arrow(&a, &b, None).unwrap();
    let version_before = element(&s, &a)["version"].as_u64().unwrap();
    assert_eq!(
        s.delete(std::slice::from_ref(&a)).unwrap(),
        3,
        "box, its label and the arrow"
    );
    assert_eq!(element(&s, &a)["isDeleted"], true);
    assert_eq!(
        element(&s, &a)["version"].as_u64().unwrap(),
        version_before + 1
    );
    let b_bound = element(&s, &b)["boundElements"].clone();
    assert!(
        !b_bound
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == arrow.as_str()),
        "B forgets the arrow"
    );
    assert_eq!(element(&s, &arrow)["isDeleted"], true);
    assert_eq!(
        s.summary().iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
        vec![b]
    );
    assert!(s.delete(&["missing".into()]).is_err());
}

#[test]
fn explicit_box_size_grows_to_fit_its_label() {
    let mut s = empty();
    let label = "Order lines: Product | Quantity | Unit price | Taxes | Subtotal (sections & notes) - Total";
    let id = s
        .add_box(label, at(0.0, 0.0), Some((180.0, 40.0)), None, None)
        .unwrap();
    let rect = element(&s, &id);
    let text_id = rect["boundElements"][0]["id"].as_str().unwrap().to_string();
    let text = element(&s, &text_id);
    let (rw, rh) = (
        rect["width"].as_f64().unwrap(),
        rect["height"].as_f64().unwrap(),
    );
    let (tw, th) = (
        text["width"].as_f64().unwrap(),
        text["height"].as_f64().unwrap(),
    );
    assert_eq!(rw, 180.0, "the requested width is kept");
    assert!(tw <= rw, "label wrapped inside the box: {tw} > {rw}");
    assert!(rh >= th + 10.0, "box grew to fit the label: {rh} < {th}");
    // The label is centred in the grown box.
    let ty = text["y"].as_f64().unwrap();
    assert!((ty + th / 2.0 - rh / 2.0).abs() < 1.0);
}

#[test]
fn a_button_sized_box_with_one_line_keeps_its_height() {
    let mut s = empty();
    let id = s
        .add_box("Confirm", at(0.0, 0.0), Some((120.0, 40.0)), None, None)
        .unwrap();
    assert_eq!(element(&s, &id)["height"].as_f64(), Some(40.0));
}

#[test]
fn size_of_reports_the_final_box_size() {
    let mut s = empty();
    let id = s
        .add_box("Confirm", at(0.0, 0.0), Some((120.0, 40.0)), None, None)
        .unwrap();
    assert_eq!(s.size_of(&id), Some((120.0, 40.0)));
    assert_eq!(s.size_of("missing"), None);
}

// ---- Annotations: coloured notes, pointers and highlights on screenshots ----

#[test]
fn notes_can_be_red() {
    let mut s = empty();
    let id = s.add_text("New field", at(0.0, 0.0), None, Some(Color::Red));
    assert_eq!(element(&s, &id)["strokeColor"], "#e03131");
    let plain = s.add_text("Title", at(0.0, 100.0), None, None);
    assert_eq!(element(&s, &plain)["strokeColor"], "#1e1e1e");
}

/// A 1000x500 screenshot shown at half size (500x250) at (100, 200).
fn scene_with_screenshot() -> (Scene, String) {
    let mut s = empty();
    let img = s
        .add_image(&png(1000, 500), at(100.0, 200.0), Some(500.0))
        .unwrap();
    (s, img)
}

#[test]
fn a_point_on_a_screenshot_is_given_in_the_screenshots_own_pixels() {
    let (s, img) = scene_with_screenshot();
    let p = s
        .resolve(&Target::OnImage {
            image: img,
            x: 400.0,
            y: 100.0,
        })
        .unwrap();
    assert_eq!(p, (100.0 + 200.0, 200.0 + 50.0));
    assert_eq!(s.resolve(&Target::Point(5.0, 6.0)).unwrap(), (5.0, 6.0));
}

#[test]
fn a_pointer_goes_from_a_note_to_a_spot_on_the_screenshot() {
    let (mut s, img) = scene_with_screenshot();
    let note = s.add_text("New: Delivery date", at(700.0, 0.0), None, Some(Color::Red));
    let id = s
        .add_pointer(
            &note,
            &Target::OnImage {
                image: img,
                x: 400.0,
                y: 100.0,
            },
            None,
        )
        .unwrap();
    let a = element(&s, &id);
    assert_eq!(a["type"], "arrow");
    assert_eq!(a["strokeColor"], "#e03131", "pointers are red by default");
    assert_eq!(a["startBinding"]["elementId"], note.as_str());
    assert!(
        a["endBinding"].is_null(),
        "the end points at a spot, not an element"
    );
    // The tip lands exactly on the spot.
    let pts = a["points"].as_array().unwrap();
    let tip = pts.last().unwrap();
    let x = a["x"].as_f64().unwrap() + tip[0].as_f64().unwrap();
    let y = a["y"].as_f64().unwrap() + tip[1].as_f64().unwrap();
    assert!(
        (x - 300.0).abs() < 0.01 && (y - 250.0).abs() < 0.01,
        "tip at ({x}, {y})"
    );
    // The note knows about its arrow, so moving it keeps them together in the editor.
    let bound = element(&s, &note)["boundElements"].clone();
    assert!(bound
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b["id"] == id.as_str()));
}

#[test]
fn a_pointer_needs_an_existing_note_and_screenshot() {
    let (mut s, _) = scene_with_screenshot();
    assert!(s
        .add_pointer("nope", &Target::Point(0.0, 0.0), None)
        .is_err());
    let note = s.add_text("n", at(0.0, 0.0), None, None);
    let bad = Target::OnImage {
        image: note.clone(),
        x: 1.0,
        y: 1.0,
    };
    assert!(
        s.add_pointer(&note, &bad, None).is_err(),
        "a text is not a screenshot"
    );
}

#[test]
fn a_highlight_outlines_part_of_a_screenshot() {
    let (mut s, img) = scene_with_screenshot();
    let id = s
        .add_highlight(
            &Target::OnImage {
                image: img,
                x: 200.0,
                y: 100.0,
            },
            (300.0, 50.0),
            None,
        )
        .unwrap();
    let h = element(&s, &id);
    assert_eq!(h["type"], "rectangle");
    assert_eq!(h["backgroundColor"], "transparent");
    assert_eq!(h["strokeColor"], "#e03131");
    // Position and size are scaled from screenshot pixels like the pointer target.
    assert_eq!(
        (h["x"].as_f64(), h["y"].as_f64()),
        (Some(200.0), Some(250.0))
    );
    assert_eq!(
        (h["width"].as_f64(), h["height"].as_f64()),
        (Some(150.0), Some(25.0))
    );
    assert!(h["boundElements"].is_null(), "no label inside a highlight");
}

#[test]
fn the_summary_gives_a_pointers_start_and_tip() {
    let (mut s, img) = scene_with_screenshot();
    let note = s.add_text("Note", at(700.0, 0.0), None, Some(Color::Red));
    let id = s
        .add_pointer(
            &note,
            &Target::OnImage {
                image: img,
                x: 400.0,
                y: 100.0,
            },
            None,
        )
        .unwrap();
    let p = s.summary().into_iter().find(|e| e.id == id).unwrap();
    // Read as "from (x, y) to end", not as a box: the tip is where it points.
    assert_eq!(p.end, Some((300.0, 250.0)));
    let note_summary = s.summary().into_iter().find(|e| e.id == note).unwrap();
    assert_eq!(note_summary.end, None);
}

#[test]
fn a_note_placed_on_another_note_moves_below_it() {
    let mut s = empty();
    let first = s.add_text(
        "Line one\nLine two\nLine three",
        at(900.0, 100.0),
        None,
        Some(Color::Red),
    );
    let first_bottom = s
        .summary()
        .into_iter()
        .find(|e| e.id == first)
        .map(|e| e.y + e.height)
        .unwrap();
    let second = s.add_text("Second note", at(900.0, 120.0), None, Some(Color::Red));
    let e = s.summary().into_iter().find(|e| e.id == second).unwrap();
    assert!(e.y >= first_bottom, "overlaps: {} < {first_bottom}", e.y);
    assert_eq!(e.x, 900.0, "stays in its column");
    // Text elsewhere is left where it was asked.
    let apart = s.add_text("Apart", at(0.0, 120.0), None, None);
    let e = s.summary().into_iter().find(|e| e.id == apart).unwrap();
    assert_eq!((e.x, e.y), (0.0, 120.0));
}

#[test]
fn overlaps_between_texts_and_boxes_are_reported() {
    let (mut s, img) = scene_with_screenshot();
    let title = s.add_text("A long title line", at(0.0, -100.0), Some(28.0), None);
    let boxed = s
        .add_box("Mockup", at(10.0, -95.0), Some((200.0, 80.0)), None, None)
        .unwrap();
    let on_shot = s.add_text("New Field", at(150.0, 250.0), None, None);
    s.add_highlight(
        &Target::OnImage {
            image: img,
            x: 100.0,
            y: 100.0,
        },
        (200.0, 50.0),
        None,
    )
    .unwrap();
    assert_eq!(s.overlapping(&boxed), vec![title.clone()]);
    assert_eq!(s.overlapping(&title), vec![boxed.clone()]);
    // Writing on a screenshot, or outlining part of it, is meant to overlap it.
    assert!(s.overlapping(&on_shot).is_empty());
}

#[test]
fn an_image_remembers_the_screenshot_it_came_from() {
    let (mut s, img) = scene_with_screenshot();
    assert_eq!(s.source_of(&img), None);
    s.set_source(&img, "screenshots/form.png").unwrap();
    assert_eq!(s.source_of(&img).as_deref(), Some("screenshots/form.png"));
    // Kept in the file, where Excalidraw keeps custom data.
    assert_eq!(
        element(&s, &img)["customData"]["source"],
        "screenshots/form.png"
    );
    let reloaded = Scene::parse(&s.to_json()).unwrap();
    assert_eq!(
        reloaded.source_of(&img).as_deref(),
        Some("screenshots/form.png")
    );
    assert!(s.set_source("nope", "x.png").is_err());
}

// ---- building blocks for laying out a whole spec ----

#[test]
fn a_panel_is_a_tinted_background_drawn_behind_everything() {
    let mut s = empty();
    let text = s.add_text("Hello", at(10.0, 10.0), None, None);
    let panel = s.add_panel((0.0, 0.0, 400.0, 200.0), Color::Blue);
    let d = doc(&s);
    let elements = d["elements"].as_array().unwrap();
    assert_eq!(
        elements[0]["id"],
        panel.as_str(),
        "first = drawn at the back"
    );
    assert_eq!(elements[1]["id"], text.as_str());
    let p = element(&s, &panel);
    assert_eq!(p["type"], "rectangle");
    assert_ne!(p["backgroundColor"], "transparent");
    assert!(p["opacity"].as_f64().unwrap() < 100.0);
    assert!(
        s.overlapping(&text).is_empty(),
        "a panel never counts as an overlap"
    );
}

#[test]
fn an_arrow_can_be_made_red_and_dashed() {
    let mut s = empty();
    let a = s.add_box("A", at(0.0, 0.0), None, None, None).unwrap();
    let b = s.add_box("B", at(600.0, 0.0), None, None, None).unwrap();
    let arrow = s.add_arrow(&a, &b, Some("rule")).unwrap();
    s.style_line(&arrow, Color::Red, true).unwrap();
    let e = element(&s, &arrow);
    assert_eq!(e["strokeStyle"], "dashed");
    assert_eq!(e["strokeColor"], "#e03131");
}

#[test]
fn tagged_elements_are_found_again_and_measured_together() {
    let mut s = empty();
    let one = s.add_text("One", at(0.0, 0.0), None, None);
    let two = s.add_text("Two", at(100.0, 300.0), None, None);
    let mine = s.add_text("Mine", at(0.0, 900.0), None, None);
    s.tag(&one, "spec").unwrap();
    s.tag(&two, "spec").unwrap();
    let mut found = s.tagged("spec");
    found.sort();
    let mut want = vec![one.clone(), two.clone()];
    want.sort();
    assert_eq!(found, want);
    assert!(!s.tagged("spec").contains(&mine));
    let (x, y, w, h) = s.bounds(&[one, two]).unwrap();
    assert_eq!((x, y), (0.0, 0.0));
    assert!(w > 100.0 && h > 300.0);
}
