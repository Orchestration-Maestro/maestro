//! One-pass markup escaping for text and XML attribute literals.

/// Entity spelling and XML attribute whitespace preservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeMode {
    /// Apostrophe as the named entity `&apos;`.
    Xml,
    /// Spreadsheet text with line breaks and hexadecimal control references.
    SpreadsheetHtml,
    /// XML entities with character references for tab, LF and CR in attributes.
    XmlAttribute,
    /// Apostrophe as a decimal numeric character reference.
    HtmlDecimal,
    /// Apostrophe as a zero-padded decimal numeric character reference.
    HtmlDecimalPadded,
    /// Apostrophe as a hexadecimal numeric character reference.
    HtmlHex,
}

/// Escape `text` in `mode` into a new string.
#[must_use]
pub fn escape(text: &str, mode: EscapeMode) -> String {
    let mut out = String::new();
    escape_into(text, &mut out, mode);
    out
}

/// Append `text` escaped in `mode` to `out`, keeping what `out` already holds.
pub fn escape_into(text: &str, out: &mut String, mode: EscapeMode) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str(match mode {
                EscapeMode::Xml | EscapeMode::XmlAttribute | EscapeMode::SpreadsheetHtml => {
                    "&apos;"
                }
                EscapeMode::HtmlDecimal => "&#39;",
                EscapeMode::HtmlDecimalPadded => "&#039;",
                EscapeMode::HtmlHex => "&#x27;",
            }),
            '\t' if mode == EscapeMode::XmlAttribute => out.push_str("&#9;"),
            '\n' if mode == EscapeMode::XmlAttribute => out.push_str("&#10;"),
            '\r' if mode == EscapeMode::XmlAttribute => out.push_str("&#13;"),
            '\n' if mode == EscapeMode::SpreadsheetHtml => out.push_str("<br/>"),
            c if mode == EscapeMode::SpreadsheetHtml && c <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(out, "&#x{:04x};", u32::from(c));
            }
            c => out.push(c),
        }
    }
}
