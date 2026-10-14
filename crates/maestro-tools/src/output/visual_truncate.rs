//! Select a visual tail after the toolkit wraps authored text.
use maestro_tui::{Component, Text};

/// Selected padded rows with the number of rows hidden before them.
pub struct VisualTruncateResult {
    /// Last rendered rows, in display order.
    pub visual_lines: Vec<String>,
    /// Rendered rows omitted from the beginning.
    pub skipped_count: usize,
}
/// Wraps with [`Text`] before selecting at most `max_visual_lines` rows.
///
/// Absent horizontal padding means zero; a zero maximum selects no rows.
#[must_use]
pub fn truncate_to_visual_lines(
    text: &str,
    max_visual_lines: usize,
    width: usize,
    padding_x: Option<usize>,
) -> VisualTruncateResult {
    let mut visual_lines =
        Text::new(text.to_owned(), padding_x.unwrap_or(0), 0, None).render(width);
    let skipped_count = visual_lines.len().saturating_sub(max_visual_lines);
    visual_lines.drain(..skipped_count);
    VisualTruncateResult {
        visual_lines,
        skipped_count,
    }
}
