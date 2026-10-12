//! One-pass markup escaping; the modes differ only in how the apostrophe is spelled.

/// How an apostrophe is written; ampersand, angle brackets and double quote are shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeMode {
    /// Apostrophe as the named entity `&apos;`.
    Xml,
    /// Apostrophe as a decimal numeric character reference.
    HtmlDecimal,
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
                EscapeMode::Xml => "&apos;",
                EscapeMode::HtmlDecimal => "&#39;",
                EscapeMode::HtmlHex => "&#x27;",
            }),
            c => out.push(c),
        }
    }
}
