//! Raw recorded lines under the file-reading policy of one report.

use maestro_text::is_whitespace_scalar;

/// Nonblank lines after removing outer whitespace from the whole file.
///
/// Costs and transcripts trim the file once and then keep each remaining line
/// as authored, so a blank line inside the file is skipped.
pub(crate) fn trimmed_lines(content: &str) -> impl Iterator<Item = &str> {
    content
        .trim_matches(is_whitespace_scalar)
        .split('\n')
        .filter(|line| !line.is_empty())
}

/// Nonblank lines that keep their leading whitespace.
///
/// Usage and tool parsing keep the authored line so a leading byte-order mark
/// reaches the parser, while a whitespace-only line is still skipped.
pub(crate) fn kept_lines(content: &str) -> impl Iterator<Item = &str> {
    content
        .split('\n')
        .filter(|line| !line.trim_matches(is_whitespace_scalar).is_empty())
}
