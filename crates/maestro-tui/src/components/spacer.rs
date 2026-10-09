//! A run of empty rows.

use std::cell::Cell;

use crate::tui::Component;

/// Renders a configurable number of empty rows, whatever the viewport width.
pub struct Spacer {
    /// Number of empty rows rendered.
    lines: Cell<usize>,
}

impl Spacer {
    /// Creates a spacer of `lines` empty rows.
    #[must_use]
    pub const fn new(lines: usize) -> Self {
        Self {
            lines: Cell::new(lines),
        }
    }

    /// Changes the number of empty rows.
    pub fn set_lines(&self, lines: usize) {
        self.lines.set(lines);
    }
}

impl Default for Spacer {
    /// A spacer of one empty row.
    fn default() -> Self {
        Self::new(1)
    }
}

impl Component for Spacer {
    fn render(&self, _width: usize) -> Vec<String> {
        vec![String::new(); self.lines.get()]
    }
}
