//! Recognition and name extraction for Unix text listings.

use std::fmt::Write as _;
use std::sync::LazyLock;

/// The entry line, in the shape the reference accepts.
static LINE: LazyLock<Option<regex::Regex>> = LazyLock::new(|| regex::Regex::new(&pattern()?).ok());

/// The entry's name, unless the line is a parent-directory link.
pub(super) fn name(line: &str) -> Option<&str> {
    let groups = LINE.as_ref()?.captures(line)?;
    let name = groups.get(21)?.as_str();
    if name == "." || name == ".." {
        return None;
    }
    // A symbolic link spells its target after the name, which the source does not observe.
    let symbolic = matches!(groups.get(1)?.as_str(), "l" | "e");
    Some(match (symbolic, name.find(" -> ")) {
        (true, Some(end)) => &name[..end],
        _ => name,
    })
}

/// Whether a line might be a Unix-style listing entry.
pub(super) fn test_line(line: &str) -> bool {
    LINE.as_ref().is_some_and(|pattern| pattern.is_match(line))
}

/// The authored pattern, whose groups keep the reference's numbering.
fn pattern() -> Option<String> {
    let whitespace = maestro_text::WHITESPACE;
    let mut pattern = String::new();
    pattern.push_str("([bcdelfmpSs-])");
    pattern
        .push_str("(((r|-)(w|-)([xsStTL-]))((r|-)(w|-)([xsStTL-]))((r|-)(w|-)([xsStTL-]?)))\\+?");
    write!(pattern, "[{whitespace}]*").ok()?;
    pattern.push_str("([0-9]+)");
    write!(pattern, "[{whitespace}]+").ok()?;
    write!(
        pattern,
        "(?:([^{whitespace}]+(?:[{whitespace}]+[^{whitespace}]+)*?)[{whitespace}]+)?"
    )
    .ok()?;
    write!(
        pattern,
        "(?:([^{whitespace}]+(?:[{whitespace}]+[^{whitespace}]+)*)[{whitespace}]+)?"
    )
    .ok()?;
    write!(
        pattern,
        "([0-9]+(?:,[{whitespace}]*[0-9]+)?)[{whitespace}]+"
    )
    .ok()?;
    write!(
        pattern,
        "((?:[0-9]+[-/][0-9]+[-/][0-9]+)|(?:[^{whitespace}]{{3}}[{whitespace}]+[0-9]{{1,2}})|(?:[0-9]{{1,2}}[{whitespace}]+[^{whitespace}]{{3}})|(?:[0-9]{{1,2}}月[{whitespace}]+[0-9]{{1,2}}日))[{whitespace}]+"
    )
    .ok()?;
    write!(
        pattern,
        "((?:[0-9]+(?::[0-9]+)?)|(?:[0-9]{{4}}年))[{whitespace}](.*)"
    )
    .ok()?;
    Some(pattern)
}
