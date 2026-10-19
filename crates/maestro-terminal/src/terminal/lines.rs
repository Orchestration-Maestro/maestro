//! Terminal tab stops and line control over the active screen.

use super::{TABSTOP_INTERVAL, Terminal};
use crate::{Mode, csi::TabClear};

impl Terminal {
    /// Move to the next tab stop, bounded by the right scrolling margin.
    pub fn horizontal_tab(&mut self) {
        let screen = self.screens.active_mut();
        while screen.cursor.x < self.scrolling_region.right {
            screen.cursor_right(1);
            if self.tabstops.get(usize::from(screen.cursor.x)) {
                return;
            }
        }
    }

    /// Move to the preceding tab stop, bounded by the origin-mode left margin.
    pub fn horizontal_tab_back(&mut self) {
        let left = if self.modes.get(Mode::Origin) {
            self.scrolling_region.left
        } else {
            0
        };
        let screen = self.screens.active_mut();
        while screen.cursor.x > left {
            screen.cursor_left(1);
            if self.tabstops.get(usize::from(screen.cursor.x)) {
                return;
            }
        }
    }

    /// Clear the current stop or all stops; ignore unsupported parameters.
    pub fn tab_clear(&mut self, cmd: TabClear) {
        match cmd {
            TabClear::Current => self
                .tabstops
                .unset(usize::from(self.screens.active().cursor.x)),
            TabClear::All => self.tabstops.reset(0),
            TabClear::Unknown(_) => {}
        }
    }

    /// Set a stop at the current cursor column.
    pub fn tab_set(&mut self) {
        self.tabstops
            .set(usize::from(self.screens.active().cursor.x));
    }

    /// Restore interval-eight destinations for the current managed width.
    pub fn tab_reset(&mut self) {
        self.tabstops.reset(TABSTOP_INTERVAL);
    }
}
