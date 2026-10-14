//! CSS string serialization shared by presentation consumers.
use std::fmt::Write as _;

/// Append the CSS escape of NUL, C0 controls, DEL or `<`, returning whether one was written.
pub fn escape_control(out: &mut String, c: char) -> bool {
    match c {
        '\0' => out.push('\u{FFFD}'),
        '\u{1}'..='\u{1F}' | '\u{7F}' | '<' => {
            let _ = write!(out, "\\{:x} ", u32::from(c));
        }
        _ => return false,
    }
    true
}

/// Serialize a CSS string, quoted, with `<` escaped for style-element safety.
#[must_use]
pub fn string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        if !escape_control(&mut out, c) {
            if matches!(c, '"' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
    }
    out.push('"');
    out
}
