//! Finding sessions by what they are called and by what was said in them.

use anyhow::Result;
use serde::Serialize;

use crate::transcript::TranscriptEntry;
use crate::workspace::{SessionMeta, Workspace};

/// A session that matches a search, with where the words were said.
#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct SearchHit {
    pub project: String,
    pub session: SessionMeta,
    /// Up to two passages of the conversation holding the words.
    pub snippets: Vec<String>,
}

/// How much text is shown around a match.
const BEFORE: usize = 40;
const AFTER: usize = 80;
const SNIPPETS: usize = 2;

/// Sessions whose title or conversation holds every word of `query`: those named
/// after it first, then the most recent. An empty query gives the recent sessions.
///
/// # Errors
/// The workspace cannot be read.
pub fn search(workspace: &Workspace, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(bool, SearchHit)> = Vec::new();
    for project in workspace.list()? {
        for session in project.sessions {
            if terms.is_empty() {
                hits.push((
                    false,
                    SearchHit {
                        project: project.name.clone(),
                        session,
                        snippets: vec![],
                    },
                ));
                continue;
            }
            let said: Vec<String> = workspace
                .read_transcript(&project.name, &session.id)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|e| match e {
                    TranscriptEntry::User { text } | TranscriptEntry::Assistant { text } => {
                        Some(text)
                    }
                    _ => None,
                })
                .collect();
            let title = session.title.to_lowercase();
            let all = format!("{title}\n{}", said.join("\n").to_lowercase());
            if !terms.iter().all(|t| all.contains(t.as_str())) {
                continue;
            }
            let in_title = terms.iter().all(|t| title.contains(t.as_str()));
            let snippets = said
                .iter()
                .filter_map(|text| snippet(text, &terms))
                .take(SNIPPETS)
                .collect();
            hits.push((
                in_title,
                SearchHit {
                    project: project.name.clone(),
                    session,
                    snippets,
                },
            ));
        }
    }
    hits.sort_by(|(a_title, a), (b_title, b)| {
        b_title
            .cmp(a_title)
            .then(b.session.updated_at.cmp(&a.session.updated_at))
    });
    Ok(hits.into_iter().take(limit).map(|(_, h)| h).collect())
}

/// The text around the first word of the search found in `text`, on one line.
fn snippet(text: &str, terms: &[String]) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    // Lowercasing can change the length of some letters: then match on the original.
    let haystack = if lower.len() == chars.len() {
        &lower
    } else {
        &chars
    };
    let at = terms.iter().find_map(|t| {
        let needle: Vec<char> = t.chars().collect();
        haystack
            .windows(needle.len().max(1))
            .position(|w| w == needle.as_slice())
    })?;
    let start = at.saturating_sub(BEFORE);
    let end = (at + AFTER).min(chars.len());
    let piece: String = chars[start..end]
        .iter()
        .map(|c| if c.is_whitespace() { ' ' } else { *c })
        .collect();
    let piece = piece.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(format!(
        "{}{piece}{}",
        if start > 0 { "…" } else { "" },
        if end < chars.len() { "…" } else { "" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(ws: &Workspace, project: &str, id: &str, user: &str, assistant: &str) {
        ws.append_transcript(
            project,
            id,
            &[
                TranscriptEntry::User { text: user.into() },
                TranscriptEntry::Assistant {
                    text: assistant.into(),
                },
            ],
        )
        .unwrap();
    }

    fn workspace() -> (tempfile::TempDir, Workspace, [String; 3]) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::new(dir.path());
        ws.create_project("Acme").unwrap();
        ws.create_project("Globex").unwrap();
        let a = ws
            .create_session("Acme", "Revise quotation", "claude")
            .unwrap()
            .id;
        let b = ws
            .create_session("Acme", "Stock report", "claude")
            .unwrap()
            .id;
        let c = ws
            .create_session("Globex", "Delivery window", "claude")
            .unwrap()
            .id;
        said(
            &ws,
            "Acme",
            &a,
            "Add a Revise Quotation button",
            "The original quotation is cancelled.",
        );
        said(
            &ws,
            "Acme",
            &b,
            "Show the expiration date on the stock report",
            "The lot's expiration_date is already printed under the barcode when it is set.",
        );
        said(
            &ws,
            "Globex",
            &c,
            "A delivery window on orders",
            "Use the commitment date field.",
        );
        (dir, ws, [a, b, c])
    }

    #[test]
    fn finds_words_said_in_a_conversation_with_where_they_were() {
        let (_dir, ws, [_, stock, _]) = workspace();
        let hits = search(&ws, "expiration barcode", 20).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            (hits[0].project.as_str(), hits[0].session.id.as_str()),
            ("Acme", stock.as_str())
        );
        assert!(
            hits[0].snippets.iter().any(|s| s.contains("expiration")),
            "{:?}",
            hits[0].snippets
        );
    }

    #[test]
    fn every_word_must_match_and_case_does_not_matter() {
        let (_dir, ws, _) = workspace();
        assert_eq!(search(&ws, "QUOTATION", 20).unwrap().len(), 1);
        assert!(search(&ws, "quotation barcode", 20).unwrap().is_empty());
    }

    #[test]
    fn sessions_named_after_the_search_come_first() {
        let (_dir, ws, [revise, _, _]) = workspace();
        // "date" is said in two sessions; none is named after it.
        assert_eq!(search(&ws, "date", 20).unwrap().len(), 2);
        // "quotation" is in the title of one: it comes first.
        assert_eq!(search(&ws, "quotation", 20).unwrap()[0].session.id, revise);
    }

    #[test]
    fn an_empty_search_lists_recent_sessions() {
        let (_dir, ws, _) = workspace();
        let hits = search(&ws, "  ", 2).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.snippets.is_empty()));
    }

    #[test]
    fn a_long_passage_is_cut_around_the_match() {
        let text = format!("{} needle {}", "a".repeat(200), "b".repeat(200));
        let s = snippet(&text, &["needle".into()]).unwrap();
        assert!(
            s.starts_with('…') && s.ends_with('…') && s.contains("needle"),
            "{s}"
        );
        assert!(s.chars().count() < 140, "{}", s.chars().count());
    }
}
