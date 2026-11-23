//! A real PDF through the real PDF tools (poppler-utils): skipped where they are missing.

use std::path::Path;

use specs_core::attachments::{attach_pdf, PdfTools};

const QUOTE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/client-quotation.pdf"
);

fn tools_here() -> bool {
    ["/usr/bin/pdftotext", "/usr/bin/pdftoppm"]
        .iter()
        .all(|p| Path::new(p).is_file())
}

#[test]
fn a_real_client_quotation_becomes_clean_text_and_a_layout_picture() {
    if !tools_here() {
        eprintln!("poppler-utils is not installed: skipped");
        return;
    }
    let session = tempfile::tempdir().unwrap();
    let a = attach_pdf(session.path(), Path::new(QUOTE), &PdfTools::default()).unwrap();
    assert_eq!((a.pages, a.scanned), (1, false));
    let md = std::fs::read_to_string(session.path().join(&a.text_file)).unwrap();
    // What matters for mapping stays: the labels, the layout, the client's fields.
    for kept in [
        "QUOTATION",
        "Customer PO Number: PO-77821",
        "Incoterm: FOB Jakarta",
        "Payment terms",
        "Item code",
        "Disc.",
        "VAT 11%",
    ] {
        assert!(md.contains(kept), "{kept} was lost:\n{md}");
    }
    // The personal data the app can recognise is gone.
    for gone in [
        "sales@sample-trading.example",
        "5550 1234",
        "0123456789",
        "1.250.000",
        "23.171.250",
        "3456 7890",
    ] {
        assert!(!md.contains(gone), "{gone} is still there:\n{md}");
    }
    assert!(a.removed >= 8, "{} removed:\n{md}", a.removed);
    // A picture of page 1, small: layout only, about 20-60 tokens for the AI.
    let png = std::fs::read(session.path().join(a.layout_image.unwrap())).unwrap();
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    assert!(width <= 200, "{width}");
    println!("{md}");
}
