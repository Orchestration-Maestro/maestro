use super::{TextPiece, ansi::AnsiCodeTracker, grapheme_width, pieces};

/// Extract visible columns; strict mode excludes a crossing right-bound grapheme.
#[must_use]
pub fn slice_by_column(
    line: &str,
    start_col: usize,
    length: usize,
    strict: Option<bool>,
) -> String {
    slice_with_width(line, start_col, length, strict).0
}
/// Extract visible columns and their measured width; tabs retain their native grapheme width here.
#[must_use]
pub fn slice_with_width(
    line: &str,
    start_col: usize,
    length: usize,
    strict: Option<bool>,
) -> (String, usize) {
    if length == 0 {
        return (String::new(), 0);
    }
    let end = start_col.saturating_add(length);
    let mut result = String::new();
    let mut pending = String::new();
    let mut width: usize = 0;
    let mut column: usize = 0;
    for piece in pieces(line) {
        match piece {
            TextPiece::Ansi(code) if column >= start_col && column < end => result.push_str(code),
            TextPiece::Ansi(code) if column < start_col => pending.push_str(code),
            TextPiece::Ansi(_) => {}
            TextPiece::Text(grapheme) => {
                let cells = grapheme_width(grapheme);
                if column >= start_col
                    && column < end
                    && (!strict.unwrap_or(false) || cells <= end - column)
                {
                    result.push_str(&pending);
                    pending.clear();
                    result.push_str(grapheme);
                    width = width.saturating_add(cells);
                }
                column = column.saturating_add(cells);
                if column >= end {
                    break;
                }
            }
        }
    }
    (result, width)
}
/// Extract before/after regions, inheriting canonical active style in the after region.
#[must_use]
pub fn extract_segments(
    line: &str,
    before_end: usize,
    after_start: usize,
    after_len: usize,
    strict_after: Option<bool>,
) -> (String, usize, String, usize) {
    let mut before = String::new();
    let mut after = String::new();
    let mut before_width: usize = 0;
    let mut after_width: usize = 0;
    let mut column: usize = 0;
    let mut pending = String::new();
    let mut after_started = false;
    let mut tracker = AnsiCodeTracker::default();
    let after_end = after_start.saturating_add(after_len);
    let stop = if after_len == 0 {
        before_end
    } else {
        after_end
    };
    for piece in pieces(line) {
        let grapheme = match piece {
            TextPiece::Ansi(code) => {
                tracker.process(code);
                if column < before_end {
                    pending.push_str(code);
                } else if column >= after_start && column < after_end && after_started {
                    after.push_str(code);
                }
                continue;
            }
            TextPiece::Text(grapheme) => grapheme,
        };
        let cells = grapheme_width(grapheme);
        if column < before_end {
            before.push_str(&pending);
            pending.clear();
            before.push_str(grapheme);
            before_width = before_width.saturating_add(cells);
        } else if column >= after_start
            && column < after_end
            && (!strict_after.unwrap_or(false) || cells <= after_end - column)
        {
            if !after_started {
                after.push_str(&tracker.get_active_codes());
                after_started = true;
            }
            after.push_str(grapheme);
            after_width = after_width.saturating_add(cells);
        }
        column = column.saturating_add(cells);
        if column >= stop {
            break;
        }
    }
    (before, before_width, after, after_width)
}
