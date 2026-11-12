//! One-based scrolling margin requests.

use super::Terminal;
use crate::Mode;

impl Terminal {
    /// Set vertical margins and home the cursor, ignoring empty or inverted ranges.
    pub fn set_top_and_bottom_margin(&mut self, top: usize, bottom: usize) {
        let top = top.max(1);
        let bottom = if bottom == 0 {
            usize::from(self.rows)
        } else {
            bottom
        }
        .min(usize::from(self.rows));
        if top >= bottom {
            return;
        }
        self.scrolling_region.top = u16::try_from(top - 1).unwrap_or(0);
        self.scrolling_region.bottom = u16::try_from(bottom - 1).unwrap_or(self.rows - 1);
        self.set_cursor_pos(1, 1);
    }

    /// Set horizontal margins and home the cursor when left/right margins are enabled.
    ///
    /// Empty or inverted ranges leave both margins and cursor unchanged.
    pub fn set_left_and_right_margin(&mut self, left: usize, right: usize) {
        if !self.modes.get(Mode::EnableLeftAndRightMargin) {
            return;
        }
        let left = left.max(1);
        let right = if right == 0 {
            usize::from(self.cols)
        } else {
            right
        }
        .min(usize::from(self.cols));
        if left >= right {
            return;
        }
        self.scrolling_region.left = u16::try_from(left - 1).unwrap_or(0);
        self.scrolling_region.right = u16::try_from(right - 1).unwrap_or(self.cols - 1);
        self.set_cursor_pos(1, 1);
    }
}
