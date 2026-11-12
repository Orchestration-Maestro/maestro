//! Tolerant date-format recognition.

use super::general::is_general;

/// Recognize elapsed brackets, including unsupported but date-like fields.
pub(super) fn is_elapsed(block: &str) -> bool {
    block
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .is_some_and(|s| s.chars().all(|c| "HhMmSsชนท".contains(c)))
}

/// Report whether a spreadsheet pattern contains calendar or clock fields.
///
/// This tolerant predicate does not validate field widths or syntax.
#[must_use]
pub fn is_date(format: &str) -> bool {
    let mut chars = format.char_indices().peekable();
    while let Some((offset, c)) = chars.next() {
        if matches!(c, 'G' | 'g') && is_general(format, offset) {
            for _ in 0..6 {
                chars.next();
            }
            continue;
        }
        match c {
            '"' => {
                chars.find(|(_, next)| *next == '"');
            }
            '\\' | '_' => {
                chars.next();
            }
            'B' | 'b' | 'M' | 'D' | 'Y' | 'H' | 'S' | 'E' | 'm' | 'd' | 'y' | 'h' | 's' | 'e'
            | 'g' => return true,
            'A' | 'a' | '上' if meridiem_pattern(&format[offset..]).is_some() => return true,
            '[' => {
                let end = chars
                    .find(|(_, next)| *next == ']')
                    .map_or(format.len(), |(i, next)| i + next.len_utf8());
                if is_elapsed(&format[offset..end]) {
                    return true;
                }
            }
            '.' | '0' | '#' => {
                while chars.peek().is_some_and(|(_, n)| "0#?.,E+-%".contains(*n)) {
                    chars.next();
                }
            }
            '?' => {
                while chars.peek().is_some_and(|(_, n)| *n == '?') {
                    chars.next();
                }
            }
            '1'..='9' => {
                while chars.peek().is_some_and(|(_, n)| n.is_ascii_digit()) {
                    chars.next();
                }
            }
            '*' if chars.peek().is_some_and(|(_, n)| matches!(n, ' ' | '*')) => {
                chars.next();
            }
            _ => {}
        }
    }
    false
}

/// Recognize the three meridiem spellings without changing supplied case.
pub(super) fn meridiem_pattern(input: &str) -> Option<&'static str> {
    ["A/P", "AM/PM", "上午/下午"].into_iter().find(|prefix| {
        input
            .get(..prefix.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
    })
}
