//! Margin-bounded character deletion and protection state.

use super::Terminal;
use crate::{
    ansi::ProtectedMode,
    page::{CellPosition, cell::Wide},
};

impl Terminal {
    /// Set cursor protection, retaining the last protocol when protection is disabled.
    pub fn set_protected_mode(&mut self, mode: ProtectedMode) {
        let screen = self.screens.active_mut();
        screen.cursor.protected = mode != ProtectedMode::Off;
        if mode != ProtectedMode::Off {
            screen.protected_mode = mode;
        }
    }

    /// Erase at least one cell to the screen edge, respecting the last ISO protection mode.
    pub fn erase_chars(&mut self, count: usize) {
        let cols = self.cols();
        let screen = self.screens.active_mut();
        let Some(pin) = screen.pages.tracked_pin(screen.cursor.page_pin).copied() else {
            return;
        };
        let start = screen.cursor.x;
        let remaining = cols - start;
        let count = u16::try_from(count.max(1).min(usize::from(remaining))).unwrap_or(remaining);
        let mut end = start + count;
        if let Some(page) = screen.pages.page(pin.node)
            && end < cols
            && page.get_cells(pin.y)[usize::from(end - 1)].wide() == Wide::Wide
        {
            end += 1;
        }
        if screen.protected_mode == ProtectedMode::Iso {
            screen.clear_unprotected_cells(pin.node, pin.y, start..end);
        } else {
            screen.split_cell_boundary(start);
            screen.split_cell_boundary(end);
            screen.clear_cells(pin.node, pin.y, start..end);
        }
        screen.cursor_reset_wrap();
        screen.cursor_mark_dirty();
    }

    /// Delete cells within the horizontal margins without moving the cursor.
    ///
    /// Zero counts and cursors outside the margins leave all state unchanged.
    pub fn delete_chars(&mut self, count: usize) {
        let screen = self.screens.active_mut();
        let x = screen.cursor.x;
        let region = self.scrolling_region;
        if count == 0 || x < region.left || x > region.right {
            return;
        }
        let remaining = region.right - x + 1;
        let count = u16::try_from(count.min(usize::from(remaining))).unwrap_or(remaining);
        for boundary in [x, x + count, region.right + 1] {
            screen.split_cell_boundary(boundary);
        }
        let Some(pin) = screen.pages.tracked_pin(screen.cursor.page_pin).copied() else {
            return;
        };
        let end = region.right + 1 - count;
        if let Some(page) = screen.pages.page_mut(pin.node) {
            for col in x..end {
                page.swap_cells(
                    CellPosition { x: col, y: pin.y },
                    CellPosition {
                        x: col + count,
                        y: pin.y,
                    },
                );
            }
        }
        screen.clear_cells(pin.node, pin.y, end..region.right + 1);
        screen.cursor_reset_wrap();
        screen.cursor_mark_dirty();
    }
}
