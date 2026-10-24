//! Recognition and name extraction for DOS text listings.

use std::sync::LazyLock;

/// The entry line, in the shape the reference accepts.
static LINE: LazyLock<Option<regex::Regex>> = LazyLock::new(|| {
    let whitespace = maestro_text::WHITESPACE;
    regex::Regex::new(&format!(
        "([^{whitespace}]+)[{whitespace}]+([^{whitespace}]+)[{whitespace}]+(?:(<DIR>)|([0-9]+))[{whitespace}]+([^{whitespace}].*)"
    ))
    .ok()
});

/// Whether a line might be a DOS-style listing entry.
pub(super) fn test_line(line: &str) -> bool {
    line.as_bytes()
        .get(..2)
        .is_some_and(|prefix| prefix.iter().all(u8::is_ascii_digit))
        && LINE.as_ref().is_some_and(|pattern| pattern.is_match(line))
}

/// The entry's name, unless the line is a parent-directory link.
pub(super) fn name(line: &str) -> Option<&str> {
    let groups = LINE.as_ref()?.captures(line)?;
    let name = groups.get(5)?.as_str();
    (name != "." && name != "..").then_some(name)
}
