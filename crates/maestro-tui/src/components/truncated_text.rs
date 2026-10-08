//! Single-line text that is cut to the viewport.

use crate::text::utils::{TruncateOptions, truncate_to_width, visible_width};
use crate::tui::Component;

/// Shows the first line of a text, truncated to the viewport, with `padding_y` blank rows
/// above and below.
pub struct TruncatedText {
    /// Source text; only its first line is shown.
    text: String,
    /// Columns of padding on each side, reduced when the viewport is narrow.
    padding_x: usize,
    /// Blank rows above and below the text.
    padding_y: usize,
}

impl TruncatedText {
    /// Creates a component for `text` with horizontal and vertical padding.
    #[must_use]
    pub const fn new(text: String, padding_x: usize, padding_y: usize) -> Self {
        Self {
            text,
            padding_x,
            padding_y,
        }
    }
}

impl Component for TruncatedText {
    fn render(&mut self, width: usize) -> Vec<String> {
        let blank = " ".repeat(width);
        let padding = self.padding_x.min(width / 2);
        let first_line = self.text.split('\n').next().unwrap_or_default();
        let shown = truncate_to_width(first_line, width - 2 * padding, TruncateOptions::default());
        let side = " ".repeat(padding);
        let line = format!("{side}{shown}{side}");
        let fill = " ".repeat(width.saturating_sub(visible_width(&line)));
        let blank_rows = std::iter::repeat_n(blank, self.padding_y);
        blank_rows
            .clone()
            .chain(std::iter::once(format!("{line}{fill}")))
            .chain(blank_rows)
            .collect()
    }
}
