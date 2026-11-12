//! Clamped terminal cursor positioning.

use super::Terminal;
use crate::Mode;

impl Terminal {
    /// Position the cursor using one-based coordinates, relative to margins in origin mode.
    pub fn set_cursor_pos(&mut self, row: usize, col: usize) {
        let (left, top, right, bottom) = if self.modes.get(Mode::Origin) {
            let r = self.scrolling_region;
            (r.left, r.top, r.right + 1, r.bottom + 1)
        } else {
            (0, 0, self.cols, self.rows)
        };
        let x = u16::try_from(
            col.max(1)
                .saturating_add(usize::from(left))
                .min(usize::from(right))
                - 1,
        )
        .unwrap_or(right - 1);
        let y = u16::try_from(
            row.max(1)
                .saturating_add(usize::from(top))
                .min(usize::from(bottom))
                - 1,
        )
        .unwrap_or(bottom - 1);
        let screen = self.screens.active_mut();
        screen.cursor.pending_wrap = false;
        if y == screen.cursor.y {
            screen.cursor_horizontal_absolute(x);
        } else {
            screen.cursor_absolute(x, y);
        }
    }
}

impl Terminal {
    /// Move upward, clamped to the top margin when starting at or below it.
    pub fn cursor_up(&mut self, count: usize) {
        let screen = self.screens.active_mut();
        screen.cursor.pending_wrap = false;
        let y = screen.cursor.y;
        let max = if y >= self.scrolling_region.top {
            y - self.scrolling_region.top
        } else {
            y
        };
        screen.cursor_up(u16::try_from(count.max(1).min(usize::from(max))).unwrap_or(max));
    }

    /// Move downward without scrolling, bounded by the bottom margin or screen edge.
    pub fn cursor_down(&mut self, count: usize) {
        let screen = self.screens.active_mut();
        screen.cursor.pending_wrap = false;
        let y = screen.cursor.y;
        let max = if y <= self.scrolling_region.bottom {
            self.scrolling_region.bottom - y
        } else {
            self.rows - y - 1
        };
        screen.cursor_down(u16::try_from(count.max(1).min(usize::from(max))).unwrap_or(max));
    }

    /// Move right without wrapping, bounded by the right margin or screen edge.
    pub fn cursor_right(&mut self, count: usize) {
        let screen = self.screens.active_mut();
        screen.cursor.pending_wrap = false;
        let x = screen.cursor.x;
        let max = if x <= self.scrolling_region.right {
            self.scrolling_region.right - x
        } else {
            self.cols - x - 1
        };
        screen.cursor_right(u16::try_from(count.max(1).min(usize::from(max))).unwrap_or(max));
    }

    /// Move left, applying normal or extended reverse wrapping when enabled.
    pub fn cursor_left(&mut self, count: usize) {
        let reverse = self.modes.get(Mode::Wraparound)
            && (self.modes.get(Mode::ReverseWrap) || self.modes.get(Mode::ReverseWrapExtended));
        let screen = self.screens.active_mut();
        let mut count = count.max(1);
        if !reverse {
            screen.cursor_left(
                u16::try_from(count.min(usize::from(screen.cursor.x))).unwrap_or(screen.cursor.x),
            );
            screen.cursor.pending_wrap = false;
            return;
        }
        if screen.cursor.pending_wrap {
            count -= 1;
            screen.cursor.pending_wrap = false;
        }
        let extended = self.modes.get(Mode::ReverseWrapExtended);
        let r = self.scrolling_region;
        let left = if screen.cursor.x < r.left { 0 } else { r.left };
        if !extended && screen.cursor.x == left && screen.cursor.y <= r.top {
            screen.cursor_absolute(left, r.top);
            return;
        }
        self.cursor_reverse_wrap(count, left, extended);
    }

    /// Traverse only requested reverse-wrap columns and crossed rows.
    fn cursor_reverse_wrap(&mut self, mut count: usize, left: u16, extended: bool) {
        let r = self.scrolling_region;
        let screen = self.screens.active_mut();
        loop {
            let max = screen.cursor.x - left;
            let amount = u16::try_from(count.min(usize::from(max))).unwrap_or(max);
            count -= usize::from(amount);
            screen.cursor_left(amount);
            if count == 0 {
                break;
            }
            if screen.cursor.y == r.top && !extended {
                break;
            }
            if screen.cursor.y == r.top {
                screen.cursor_absolute(r.right, r.bottom);
                count -= 1;
                continue;
            }
            if screen.cursor.y == 0 {
                break;
            }
            if !extended && !screen.cursor_row_up(1).is_some_and(|row| row.wrap()) {
                break;
            }
            screen.cursor_absolute(r.right, screen.cursor.y - 1);
            count -= 1;
        }
    }

    /// Save position, style, protection, origin, wrap and charset on the active screen.
    pub fn save_cursor(&mut self) {
        let screen = self.screens.active_mut();
        screen.saved_cursor = Some(crate::screen::SavedCursor {
            x: screen.cursor.x,
            y: screen.cursor.y,
            style: screen.cursor.style,
            protected: screen.cursor.protected,
            pending_wrap: screen.cursor.pending_wrap,
            origin: self.modes.get(Mode::Origin),
            charset: screen.charset,
        });
    }

    /// Restore the active screen's saved cursor, or its initial defaults if absent.
    pub fn restore_cursor(&mut self) {
        let screen = self.screens.active_mut();
        let saved = screen.saved_cursor.unwrap_or(crate::screen::SavedCursor {
            x: 0,
            y: 0,
            style: crate::Style::default(),
            protected: false,
            pending_wrap: false,
            origin: false,
            charset: crate::screen::CharsetState::default(),
        });
        screen.cursor.style = saved.style;
        if screen.manual_style_update().is_err() {
            screen.cursor.style = crate::Style::default();
            let _ = screen.manual_style_update();
        }
        screen.charset = saved.charset;
        self.modes.set(Mode::Origin, saved.origin);
        screen.cursor.pending_wrap = saved.pending_wrap;
        screen.cursor.protected = saved.protected;
        screen.cursor_absolute(saved.x.min(self.cols - 1), saved.y.min(self.rows - 1));
    }
}
