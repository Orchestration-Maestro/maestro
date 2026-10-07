use super::{
    TextPiece, ansi::AnsiCodeTracker, grapheme_width, is_printable_ascii, pieces, visible_width,
};

/// Truncate to columns; defaults to `...` and no padding.
#[must_use]
pub fn truncate_to_width(
    text: &str,
    max: usize,
    ellipsis: Option<&str>,
    pad: Option<bool>,
) -> String {
    if max == 0 {
        return String::new();
    }
    let pad = pad.unwrap_or(false);
    let ellipsis = ellipsis.unwrap_or("...");
    let ew = visible_width(ellipsis);
    if ew >= max {
        let width = visible_width(text);
        if width <= max {
            return padded(text.into(), max - width, pad);
        }
        let (clipped, width) = truncate_fragment_to_width(ellipsis, max);
        return if width == 0 {
            padded(String::new(), max, pad)
        } else {
            finalize_truncated_result(("", 0), (&clipped, width), max, pad)
        };
    }
    let width = if is_printable_ascii(text) {
        text.len()
    } else {
        let mut width: usize = 0;
        for piece in pieces(text) {
            let TextPiece::Text(grapheme) = piece else {
                continue;
            };
            width = width.saturating_add(truncation_width(grapheme));
            if width > max {
                break;
            }
        }
        width
    };
    if width <= max {
        return padded(text.into(), max - width, pad);
    }
    let (prefix, pw) = truncate_fragment_to_width(text, max - ew);
    finalize_truncated_result((&prefix, pw), (ellipsis, ew), max, pad)
}

fn truncation_width(grapheme: &str) -> usize {
    if grapheme == "\t" {
        3
    } else {
        grapheme_width(grapheme)
    }
}

fn padded(mut text: String, count: usize, pad: bool) -> String {
    if pad {
        text.push_str(&" ".repeat(count));
    }
    text
}

fn truncate_fragment_to_width(text: &str, max: usize) -> (String, usize) {
    if max == 0 {
        return (String::new(), 0);
    }
    if is_printable_ascii(text) {
        let clipped = &text[..max.min(text.len())];
        return (clipped.into(), clipped.len());
    }
    let mut result = String::new();
    let mut pending = String::new();
    let mut width: usize = 0;
    for piece in pieces(text) {
        match piece {
            TextPiece::Ansi(code) => pending.push_str(code),
            TextPiece::Text(grapheme) => {
                let cells = truncation_width(grapheme);
                if cells > max.saturating_sub(width) {
                    break;
                }
                result.push_str(&pending);
                pending.clear();
                result.push_str(grapheme);
                width += cells;
            }
        }
    }
    (result, width)
}

fn finalize_truncated_result(
    (prefix, pw): (&str, usize),
    (ellipsis, ew): (&str, usize),
    max: usize,
    pad: bool,
) -> String {
    let mut result = String::new();
    for fragment in std::iter::once(prefix).chain((!ellipsis.is_empty()).then_some(ellipsis)) {
        result.push_str(fragment);
        let mut tracker = AnsiCodeTracker::default();
        tracker.update(fragment);
        tracker.close_hyperlink(&mut result);
        result.push_str("\x1b[0m");
    }
    padded(result, max.saturating_sub(pw.saturating_add(ew)), pad)
}
