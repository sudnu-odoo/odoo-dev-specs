use excalidraw::{Color, Placement, Scene};
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
    s.add_text("Title", Placement::default(), None);
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
    let id = fresh.add_text("T", Placement::default(), None);
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
    s.add_text("Note", at(0.0, 500.0), None);
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
