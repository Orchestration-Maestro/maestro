//! Newline conversion shared by text storage and parsing callers.

/// Convert every CRLF pair and lone CR to LF, leaving all other scalars untouched.
#[must_use]
pub fn normalize_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
