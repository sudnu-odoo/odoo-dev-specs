//! Writes a diagram using every kind of element this crate makes. The UI test
//! `ui/src/diagram/compat.test.ts` loads it with Excalidraw's own `restore`, to prove
//! the editor accepts what the assistant's tools write. `make test` runs Rust first.

use std::path::PathBuf;

use excalidraw::{Color, Placement, Scene};

const EMPTY: &str = r##"{"type":"excalidraw","version":2,"source":"x","elements":[],"appState":{"viewBackgroundColor":"#ffffff"},"files":{}}"##;

pub fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-fixtures/generated.excalidraw")
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    bytes.extend(b"IHDR");
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0]);
    bytes
}

#[test]
fn write_sample_for_the_ui_compatibility_test() {
    let mut s = Scene::parse(EMPTY).unwrap();
    let frame = s.add_frame(
        "Sales order form",
        Placement {
            x: Some(0.0),
            y: Some(0.0),
        },
        (900.0, 500.0),
    );
    let order = s
        .add_box(
            "Sales order",
            Placement {
                x: Some(40.0),
                y: Some(60.0),
            },
            None,
            Some(Color::Blue),
            Some(&frame),
        )
        .unwrap();
    let delivery = s
        .add_box(
            "Delivery window\n(new field)",
            Placement {
                x: Some(400.0),
                y: Some(60.0),
            },
            None,
            Some(Color::Green),
            Some(&frame),
        )
        .unwrap();
    s.add_arrow(&order, &delivery, Some("adds")).unwrap();
    s.add_text("Spec: delivery window", Placement::default(), Some(28.0));
    s.add_image(&png(1280, 720), Placement::default(), Some(640.0))
        .unwrap();
    let temp = s
        .add_box(
            "To delete",
            Placement::default(),
            None,
            Some(Color::Red),
            None,
        )
        .unwrap();
    s.delete(&[temp]).unwrap();

    let path = fixture_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, s.to_json()).unwrap();
}
