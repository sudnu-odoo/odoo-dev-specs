//! Finding something on a screenshot by name, from the map recorded when it was taken.
//!
//! An anchor is a field's technical name (`validity_date`), optionally with a kind
//! (`label:validity_date`, `field:`, `button:`, `column:`, `tab:`, `text:`, or `row:`
//! for a field together with its label), or the visible text (`Expiration`).

use anyhow::{bail, Result};

use crate::browser::Element;

/// A box on the screenshot, in its own pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Area {
    fn of(e: &Element) -> Area {
        Area {
            x: e.x,
            y: e.y,
            w: e.w,
            h: e.h,
        }
    }

    fn union(self, o: Area) -> Area {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Area {
            x,
            y,
            w: (self.x + self.w).max(o.x + o.w) - x,
            h: (self.y + self.h).max(o.y + o.h) - y,
        }
    }
}

/// Kinds tried, in order, for a bare name: what an annotation usually means.
const NAME_ORDER: [&str; 5] = ["field", "column", "button", "label", "tab"];

/// Where `anchor` is on the screenshot described by `elements`.
pub fn locate(elements: &[Element], anchor: &str) -> Result<Area> {
    let anchor = anchor.trim();
    let (kind, wanted) = match anchor.split_once(':') {
        Some((k, rest)) if is_kind(k) => (Some(k), rest.trim()),
        _ => (None, anchor),
    };
    let by_name = |kind: &str| {
        elements
            .iter()
            .find(|e| e.kind == kind && !e.name.is_empty() && e.name == wanted)
    };
    let found = match kind {
        Some("row") => {
            let parts: Vec<Area> = ["label", "field"]
                .iter()
                .filter_map(|k| by_name(k))
                .map(Area::of)
                .collect();
            parts.into_iter().reduce(Area::union)
        }
        Some("text") => by_text(elements, wanted, None)?,
        Some(kind) => match by_name(kind) {
            Some(e) => Some(Area::of(e)),
            None => by_text(elements, wanted, Some(kind))?,
        },
        None => match NAME_ORDER.iter().find_map(|k| by_name(k)) {
            Some(e) => Some(Area::of(e)),
            None => by_text(elements, wanted, None)?,
        },
    };
    match found {
        Some(area) => Ok(area),
        None => bail!(
            "Nothing called \"{anchor}\" on this screenshot. Available: {}.",
            available(elements)
        ),
    }
}

fn is_kind(k: &str) -> bool {
    matches!(
        k,
        "field" | "label" | "button" | "column" | "tab" | "text" | "row"
    )
}

/// Visible text, exact (ignoring case) first, then contained.
/// The element showing `wanted`: an exact text before one that only contains it, and a
/// real element (button, field, label…) before loose text such as a chatter line. Two
/// equally good candidates are refused, naming them, rather than guessed between.
fn by_text(elements: &[Element], wanted: &str, kind: Option<&str>) -> Result<Option<Area>> {
    let lower = wanted.to_lowercase();
    if lower.is_empty() {
        return Ok(None);
    }
    let of_kind = |e: &&Element| kind.is_none_or(|k| e.kind == k);
    let rank = |e: &Element| match e.kind.as_str() {
        "button" => 0,
        "field" | "label" => 1,
        "column" | "tab" => 2,
        _ => 3,
    };
    for exact in [true, false] {
        let hits: Vec<&Element> = elements
            .iter()
            .filter(of_kind)
            .filter(|e| {
                let t = e.text.to_lowercase();
                if exact {
                    t == lower
                } else {
                    t.contains(&lower)
                }
            })
            .collect();
        let Some(best) = hits.iter().map(|e| rank(e)).min() else {
            continue;
        };
        let top: Vec<&&Element> = hits.iter().filter(|e| rank(e) == best).collect();
        if top.len() > 1 {
            let names: Vec<String> = top
                .iter()
                .take(4)
                .map(|e| {
                    if e.name.is_empty() {
                        format!("{}:\"{}\"", e.kind, e.text)
                    } else {
                        format!("{}:{}", e.kind, e.name)
                    }
                })
                .collect();
            bail!("\"{wanted}\" matches several things on this screenshot ({}): use one of those names.", names.join(", "));
        }
        return Ok(top.first().map(|e| Area::of(e)));
    }
    Ok(None)
}

/// A short list of what can be pointed at, to help the assistant pick a valid anchor.
fn available(elements: &[Element]) -> String {
    let mut names: Vec<String> = elements
        .iter()
        .filter(|e| e.kind != "text")
        .map(|e| {
            if e.name.is_empty() {
                format!("{}:\"{}\"", e.kind, e.text)
            } else {
                format!("{}:{}", e.kind, e.name)
            }
        })
        .collect();
    if names.is_empty() {
        names = elements.iter().map(|e| format!("\"{}\"", e.text)).collect();
    }
    names.dedup();
    let shown: Vec<String> = names.iter().take(40).cloned().collect();
    let more = names.len().saturating_sub(shown.len());
    if more > 0 {
        format!("{} and {more} more", shown.join(", "))
    } else if shown.is_empty() {
        "nothing (no map for this screenshot)".into()
    } else {
        shown.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(kind: &str, name: &str, text: &str, x: f64, y: f64, w: f64, h: f64) -> Element {
        Element {
            kind: kind.into(),
            name: name.into(),
            text: text.into(),
            x,
            y,
            w,
            h,
        }
    }

    #[test]
    fn a_button_wins_over_the_same_words_in_the_chatter() {
        let els = vec![
            el("text", "", "Approve", 900.0, 400.0, 60.0, 16.0),
            el(
                "button",
                "action_approve",
                "Approve",
                100.0,
                120.0,
                80.0,
                30.0,
            ),
        ];
        assert_eq!(locate(&els, "Approve").unwrap().x, 100.0);
    }

    #[test]
    fn two_equal_candidates_are_refused_with_their_names() {
        let els = vec![
            el("button", "action_a", "Approve", 100.0, 120.0, 80.0, 30.0),
            el("button", "action_b", "Approve", 300.0, 120.0, 80.0, 30.0),
        ];
        let err = locate(&els, "Approve").unwrap_err().to_string();
        assert!(
            err.contains("button:action_a") && err.contains("button:action_b"),
            "{err}"
        );
        assert_eq!(locate(&els, "button:action_b").unwrap().x, 300.0);
    }

    #[test]
    fn a_partial_text_must_be_unique() {
        let els = vec![
            el(
                "text",
                "",
                "Grant Approval for Manager 1",
                900.0,
                400.0,
                60.0,
                16.0,
            ),
            el(
                "text",
                "",
                "Grant Approval for Manager 2",
                900.0,
                440.0,
                60.0,
                16.0,
            ),
        ];
        assert!(locate(&els, "Grant Approval").is_err());
        assert_eq!(locate(&els, "Manager 2").unwrap().y, 440.0);
    }

    fn form() -> Vec<Element> {
        vec![
            el(
                "label",
                "validity_date",
                "Expiration",
                470.0,
                240.0,
                80.0,
                20.0,
            ),
            el(
                "field",
                "validity_date",
                "Dec 17",
                590.0,
                236.0,
                295.0,
                24.0,
            ),
            el(
                "button",
                "action_confirm",
                "Confirm",
                147.0,
                116.0,
                82.0,
                36.0,
            ),
            el("button", "", "Send", 16.0, 116.0, 62.0, 36.0),
            el("column", "product_id", "Product", 20.0, 400.0, 300.0, 30.0),
        ]
    }

    #[test]
    fn a_bare_name_means_the_field() {
        let a = locate(&form(), "validity_date").unwrap();
        assert_eq!((a.x, a.y, a.w, a.h), (590.0, 236.0, 295.0, 24.0));
    }

    #[test]
    fn a_kind_picks_the_label_button_or_column() {
        assert_eq!(locate(&form(), "label:validity_date").unwrap().x, 470.0);
        assert_eq!(locate(&form(), "button:action_confirm").unwrap().x, 147.0);
        assert_eq!(locate(&form(), "column:product_id").unwrap().y, 400.0);
    }

    #[test]
    fn a_row_is_the_label_and_its_field_together() {
        let a = locate(&form(), "row:validity_date").unwrap();
        assert_eq!((a.x, a.y), (470.0, 236.0));
        assert_eq!((a.w, a.h), (885.0 - 470.0, 24.0));
    }

    #[test]
    fn visible_text_works_too() {
        assert_eq!(locate(&form(), "Expiration").unwrap().x, 470.0);
        assert_eq!(locate(&form(), "send").unwrap().x, 16.0, "ignores case");
        assert_eq!(locate(&form(), "button:Send").unwrap().x, 16.0);
        assert_eq!(locate(&form(), "text:Confirm").unwrap().x, 147.0);
    }

    #[test]
    fn an_unknown_anchor_lists_what_is_there() {
        let err = locate(&form(), "partner_id").unwrap_err().to_string();
        assert!(err.contains("partner_id"), "{err}");
        assert!(err.contains("field:validity_date"), "{err}");
        assert!(err.contains("button:\"Send\""), "{err}");
    }

    #[test]
    fn a_report_is_pointed_at_by_its_texts() {
        let report = vec![el(
            "text",
            "",
            "Quotation # S00003",
            60.0,
            500.0,
            220.0,
            24.0,
        )];
        assert_eq!(locate(&report, "Quotation #").unwrap().y, 500.0);
        let err = locate(&report, "nope").unwrap_err().to_string();
        assert!(err.contains("\"Quotation # S00003\""), "{err}");
    }
}
