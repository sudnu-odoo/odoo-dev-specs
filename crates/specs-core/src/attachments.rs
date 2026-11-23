//! Files the person attaches to a message, made cheap and safe for the AI.
//!
//! A PDF report becomes a Markdown file with each page's text in its original
//! columns (`pdftotext -layout`), plus a tiny picture of page 1 that shows the
//! layout but not the words. Personal data the app recognises is removed from the
//! text. The original PDF is never kept in the session folder, which the AI reads.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use anyhow::{bail, ensure, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Folder of the session where attachments are kept, relative to the session.
pub const FOLDER: &str = "attachments";
/// Width of the layout picture: enough to see the blocks, too small to read.
const LAYOUT_WIDTH: u32 = 200;

/// An attached file, as the chat shows it and the AI is told about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Attachment {
    /// The file the person chose, e.g. "quotation.pdf".
    pub name: String,
    /// The text for the AI, relative to the session, e.g. "attachments/quotation.md".
    pub text_file: String,
    /// The layout picture, relative to the session.
    pub layout_image: Option<String>,
    pub pages: u32,
    /// How many pieces of personal data were removed.
    pub removed: u32,
    /// Its pages are pictures without text (a scan): only the layout can be used.
    pub scanned: bool,
}

/// The poppler tools that read PDFs (Ubuntu's poppler-utils).
#[derive(Debug, Clone)]
pub struct PdfTools {
    pub pdftotext: PathBuf,
    pub pdftoppm: PathBuf,
}

impl Default for PdfTools {
    fn default() -> Self {
        Self {
            pdftotext: "pdftotext".into(),
            pdftoppm: "pdftoppm".into(),
        }
    }
}

/// Turn the PDF at `pdf` into the session's attachment files.
///
/// # Errors
/// The file is not a PDF, the PDF tools are missing, or the files cannot be written.
pub fn attach_pdf(session_dir: &Path, pdf: &Path, tools: &PdfTools) -> Result<Attachment> {
    let name = pdf
        .file_name()
        .context("That is not a file.")?
        .to_string_lossy()
        .into_owned();
    ensure!(
        name.to_lowercase().ends_with(".pdf"),
        "Only PDF files can be attached for now."
    );
    let stem = safe_stem(&name);
    let folder = session_dir.join(FOLDER);
    std::fs::create_dir_all(&folder)?;

    let text = run(
        &tools.pdftotext,
        &["-layout".as_ref(), pdf.as_os_str(), "-".as_ref()],
    )
    .context("Could not read the PDF's text. Is poppler-utils installed?")?;
    // pdftotext ends every page with a form feed.
    let pages: Vec<&str> = text.trim_end_matches('\u{c}').split('\u{c}').collect();
    let letters: usize = pages
        .iter()
        .map(|p| p.chars().filter(|c| c.is_alphanumeric()).count())
        .sum();
    let scanned = letters < 20 * pages.len();

    let mut removed = 0;
    let mut md = format!(
        "# {name}\n\nA report the person attached ({} page{}), as text in its original columns. \
Personal data the app recognised is replaced by [email], [phone], [amount], [tax id], \
[bank account] or [number].\n",
        pages.len(),
        if pages.len() == 1 { "" } else { "s" }
    );
    if scanned {
        md.push_str(
            "\nThis PDF is a scan: its pages are pictures without text. Only the layout \
picture can be used; ask the person what its fields are.\n",
        );
    } else {
        for (i, page) in pages.iter().enumerate() {
            let (clean, n) = redact(page);
            removed += n;
            md.push_str(&format!(
                "\n## Page {}\n\n```text\n{}\n```\n",
                i + 1,
                clean.trim_end()
            ));
        }
    }
    let text_file = format!("{FOLDER}/{stem}.md");
    std::fs::write(session_dir.join(&text_file), md)?;

    let layout = format!("{FOLDER}/{stem}-layout");
    let layout_image = run(
        &tools.pdftoppm,
        &[
            "-png".as_ref(),
            "-singlefile".as_ref(),
            "-f".as_ref(),
            "1".as_ref(),
            "-l".as_ref(),
            "1".as_ref(),
            "-scale-to".as_ref(),
            LAYOUT_WIDTH.to_string().as_ref(),
            pdf.as_os_str(),
            session_dir.join(&layout).as_os_str(),
        ],
    )
    .ok()
    .map(|_| format!("{layout}.png"))
    .filter(|png| session_dir.join(png).is_file());

    Ok(Attachment {
        name,
        text_file,
        layout_image,
        pages: pages.len() as u32,
        removed,
        scanned,
    })
}

/// What the AI is told about the attachments of a message.
pub fn mention(attachments: &[Attachment]) -> String {
    let lines: Vec<String> = attachments
        .iter()
        .map(|a| {
            let layout = a
                .layout_image
                .as_ref()
                .map(|p| format!("; its layout (page 1, words unreadable on purpose): {p}"))
                .unwrap_or_default();
            format!("- {}: the text is in {}{layout}", a.name, a.text_file)
        })
        .collect();
    format!(
        "[Attached reports, personal data removed:\n{}]",
        lines.join("\n")
    )
}

/// Remove the personal data the app can recognise. Returns the text and how many
/// pieces were removed. Labels, dates and the columns stay, so the report's layout
/// can still be mapped.
pub fn redact(text: &str) -> (String, u32) {
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            (r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", "[email]"),
            // Tax / VAT numbers like BE0477472701 or US12345675.
            (r"\b[A-Z]{2} ?\d{8,12}\b", "[tax id]"),
            // IBAN: country, check digits, then groups.
            (
                r"\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,4})?\b",
                "[bank account]",
            ),
            // Other long numbers (accounts, references of people).
            (r"\b\d{8,}\b", "[number]"),
            // Amounts with a currency, or with decimals / thousands separators.
            (
                r"(?:[$€£¥₹]|\b(?:Rp|IDR|USD|EUR)\b\.?) ?\d[\d.,]*",
                "[amount]",
            ),
            (r"\b\d{1,3}(?:[.,]\d{3})+(?:[.,]\d{1,2})?\b", "[amount]"),
            (r"\b\d+[.,]\d{2}\b", "[amount]"),
            // Phones: +32 2 290 34 90, (555) 123-4567, 0812-3456-7890. Each group is
            // separated by one space, dot or dash, so "S00002" or a column of
            // numbers is never taken for one.
            (
                r"(?:\+\d{1,3}[ .-]?\(?\d{1,4}\)?|\(\d{2,4}\)|\b\d{2,4})(?:[ .-]\d{2,4}){2,4}\b",
                "[phone]",
            ),
        ]
        .into_iter()
        .map(|(re, by)| (Regex::new(re).expect("valid pattern"), by))
        .collect()
    });
    let mut out = text.to_string();
    let mut count = 0;
    for (re, by) in rules {
        let mut n = 0;
        out = re
            .replace_all(&out, |_: &regex::Captures| {
                n += 1;
                by
            })
            .into_owned();
        count += n;
    }
    (out, count)
}

/// A file name the session folder can hold: letters, digits, - and _.
fn safe_stem(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let clean = clean.trim_matches('-');
    if clean.is_empty() {
        "attachment".into()
    } else {
        clean.chars().take(60).collect()
    }
}

fn run(program: &Path, args: &[&std::ffi::OsStr]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("{} is not installed.", program.display()))?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const QUOTATION: &str = "\
Deco Addict                                   Quotation # S00002
Contact: Ana Sousa  ana.sousa@example.com  Phone: +32 2 290 34 90
VAT: BE0477472701           IBAN BE71 0961 2345 6769
Quotation Date   Expiration     Salesperson
10/20/2026       11/19/2026     Mitchell Admin
Description              Quantity   Unit Price   Taxes   Amount
Office Chair                  3.00       295.00     15%   $ 885.00
Desk                          1.00     1,200.50     15%   1.200,50 €
                                        Total             Rp 1.250.000
Reference 1234567890";

    #[test]
    fn personal_data_goes_and_the_report_shape_stays() {
        let (clean, n) = redact(QUOTATION);
        for gone in [
            "ana.sousa@example.com",
            "+32",
            "290 34 90",
            "BE0477472701",
            "BE71 0961",
            "885.00",
            "1,200.50",
            "1.200,50",
            "1.250.000",
            "1234567890",
            "295.00",
        ] {
            assert!(!clean.contains(gone), "{gone} is still in:\n{clean}");
        }
        for kept in [
            "Quotation # S00002",
            "Quotation Date   Expiration     Salesperson",
            "10/20/2026",
            "Description              Quantity   Unit Price   Taxes   Amount",
            "15%",
            "Total",
        ] {
            assert!(clean.contains(kept), "{kept} was lost:\n{clean}");
        }
        assert!(n >= 10, "{n} removed:\n{clean}");
        assert!(
            clean.contains("VAT: [tax id]") && clean.contains("IBAN [bank account]"),
            "{clean}"
        );
        assert!(clean.contains("Reference [number]"), "{clean}");
    }

    #[test]
    fn names_are_left_for_the_person_to_remove() {
        // No rule can tell a person's name from a product's: the reminder covers it.
        let (clean, _) = redact("Customer: Deco Addict");
        assert_eq!(clean, "Customer: Deco Addict");
    }

    fn fake(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn tools(dir: &Path, text: &str) -> PdfTools {
        PdfTools {
            pdftotext: fake(dir, "pdftotext", &format!("printf '{text}'")),
            // Writes <prefix>.png, the last argument.
            pdftoppm: fake(
                dir,
                "pdftoppm",
                "for last; do :; done; printf 'PNG' > \"$last.png\"",
            ),
        }
    }

    #[test]
    fn a_pdf_becomes_markdown_and_a_layout_picture_and_is_not_kept() {
        let dir = tempfile::tempdir().unwrap();
        let session = dir.path().join("session");
        std::fs::create_dir_all(&session).unwrap();
        let pdf = dir.path().join("Client quotation (v2).pdf");
        std::fs::write(&pdf, "%PDF").unwrap();
        let tools = tools(
            dir.path(),
            "Quotation # S00002\\nMail: a@b.co\\n\\fPage two terms and conditions text\\n\\f",
        );
        let a = attach_pdf(&session, &pdf, &tools).unwrap();
        assert_eq!(a.name, "Client quotation (v2).pdf");
        assert_eq!(a.text_file, "attachments/Client-quotation--v2.md");
        assert_eq!(
            a.layout_image.as_deref(),
            Some("attachments/Client-quotation--v2-layout.png")
        );
        assert_eq!((a.pages, a.removed, a.scanned), (2, 1, false));
        let md = std::fs::read_to_string(session.join(&a.text_file)).unwrap();
        assert!(
            md.contains("## Page 2") && md.contains("[email]") && !md.contains("a@b.co"),
            "{md}"
        );
        // Only the cleaned files are in the session: never the PDF itself.
        let files: Vec<String> = std::fs::read_dir(session.join(FOLDER))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(files.iter().all(|f| !f.ends_with(".pdf")), "{files:?}");
        assert!(mention(&[a]).contains("attachments/Client-quotation--v2.md"));
    }

    #[test]
    fn a_scan_is_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("scan.pdf");
        std::fs::write(&pdf, "%PDF").unwrap();
        let a = attach_pdf(dir.path(), &pdf, &tools(dir.path(), "\\n \\n\\f")).unwrap();
        assert!(a.scanned);
        let md = std::fs::read_to_string(dir.path().join(&a.text_file)).unwrap();
        assert!(md.contains("This PDF is a scan"), "{md}");
    }

    #[test]
    fn only_pdfs_are_taken_and_missing_tools_are_explained() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("notes.docx");
        std::fs::write(&doc, "x").unwrap();
        let err = attach_pdf(dir.path(), &doc, &PdfTools::default()).unwrap_err();
        assert!(err.to_string().contains("Only PDF"), "{err}");
        let pdf = dir.path().join("a.pdf");
        std::fs::write(&pdf, "%PDF").unwrap();
        let none = PdfTools {
            pdftotext: "/nonexistent/pdftotext".into(),
            pdftoppm: "/nonexistent/pdftoppm".into(),
        };
        let err = attach_pdf(dir.path(), &pdf, &none).unwrap_err();
        assert!(format!("{err:#}").contains("poppler-utils"), "{err:#}");
    }
}
