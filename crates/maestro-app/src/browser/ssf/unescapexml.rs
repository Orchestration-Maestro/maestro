//! One-pass spreadsheet escapes over already XML-decoded text.
/// Decode raw spreadsheet string escapes in one pass over UTF-16 code units.
pub(super) fn unescapexml(value: &str) -> String {
    let mut rest = value;
    let mut units = Vec::new();
    while let Some(c) = rest.chars().next() {
        let escaped = rest
            .get(..7)
            .filter(|s| {
                (s.starts_with("_x") || s.starts_with("_X"))
                    && s.ends_with('_')
                    && s[2..6].bytes().all(|b| b.is_ascii_hexdigit())
            })
            .and_then(|s| u16::from_str_radix(&s[2..6], 16).ok());
        if let Some(unit) = escaped {
            units.push(unit);
            rest = &rest[7..];
        } else {
            units.extend_from_slice(c.encode_utf16(&mut [0; 2]));
            rest = &rest[c.len_utf8()..];
        }
    }
    String::from_utf16_lossy(&units)
}
