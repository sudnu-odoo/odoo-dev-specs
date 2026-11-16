//! Approximate text measuring; Excalidraw re-measures exactly when it renders.

pub const FONT_SIZE: f64 = 20.0;
pub const LINE_HEIGHT: f64 = 1.25;
/// Average glyph width relative to the font size for the hand-drawn font, on the
/// generous side: the UI re-measures with the real font, an overestimate only adds space.
const CHAR_WIDTH: f64 = 0.62;

/// Width and height of (already wrapped) text.
pub fn measure(text: &str, font_size: f64) -> (f64, f64) {
    let lines: Vec<&str> = text.split('\n').collect();
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    (
        (longest.max(1) as f64 * font_size * CHAR_WIDTH).ceil(),
        (lines.len() as f64 * font_size * LINE_HEIGHT).ceil(),
    )
}

/// Wrap words so no line is wider than `max_width` (long words are kept whole).
pub fn wrap(text: &str, max_width: f64, font_size: f64) -> String {
    let max_chars = ((max_width / (font_size * CHAR_WIDTH)).floor() as usize).max(1);
    text.split('\n')
        .map(|paragraph| wrap_line(paragraph, max_chars))
        .collect::<Vec<_>>()
        .join("\n")
}

fn wrap_line(line: &str, max_chars: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let needed =
            current.chars().count() + usize::from(!current.is_empty()) + word.chars().count();
        if !current.is_empty() && needed > max_chars {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    out.push(current);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_word_boundaries() {
        assert_eq!(
            wrap("aaa bbb ccc", 8.0 * FONT_SIZE * CHAR_WIDTH, FONT_SIZE),
            "aaa bbb\nccc"
        );
        assert_eq!(wrap("short", 1000.0, FONT_SIZE), "short");
        assert_eq!(wrap("one\ntwo", 1000.0, FONT_SIZE), "one\ntwo");
    }

    #[test]
    fn measures_lines() {
        let (w1, h1) = measure("abc", FONT_SIZE);
        let (w2, h2) = measure("abc\nabcdef", FONT_SIZE);
        assert!(w2 > w1);
        assert_eq!(h2, h1 * 2.0);
    }
}
