//! Active-screen reads and viewport scrolling.

use super::Terminal;
use crate::{Scrollbar, kitty::key::Flags, page_list::Scroll, screen_set::Key};

/// Viewport destination within retained rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollViewport {
    /// Oldest retained row.
    Top,
    /// Editable area.
    Bottom,
    /// Relative signed row displacement.
    Delta(isize),
    /// Absolute retained row offset, clamped to the editable area.
    Row(usize),
}

impl Terminal {
    /// Identity of the selected screen.
    #[must_use]
    pub fn active_screen(&self) -> Key {
        self.screens.active_key()
    }

    /// Retained row count for the selected screen.
    #[must_use]
    pub fn total_rows(&self) -> usize {
        self.screens.active().pages.total_rows()
    }

    /// Scrollable interval for the selected screen.
    pub fn scrollbar(&mut self) -> Scrollbar {
        self.screens.active_mut().pages.scrollbar()
    }

    /// Current enhanced keyboard flags for the selected screen.
    #[must_use]
    pub fn kitty_keyboard_flags(&self) -> Flags {
        self.screens.active().kitty_keyboard.current()
    }

    /// Move the selected screen's viewport without changing its content.
    pub fn scroll_viewport(&mut self, behavior: ScrollViewport) {
        self.screens.active_mut().scroll(match behavior {
            ScrollViewport::Top => Scroll::Top,
            ScrollViewport::Bottom => Scroll::Active,
            ScrollViewport::Delta(delta) => Scroll::DeltaRow(delta),
            ScrollViewport::Row(row) => Scroll::Row(row),
        });
    }
}

impl Terminal {
    /// Whether any mouse tracking protocol is enabled, independently of the selected event.
    #[must_use]
    pub fn mouse_tracking_enabled(&self) -> bool {
        use crate::Mode;
        [
            Mode::MouseEventX10,
            Mode::MouseEventNormal,
            Mode::MouseEventButton,
            Mode::MouseEventAny,
        ]
        .into_iter()
        .any(|mode| self.modes.get(mode))
    }

    /// Whether numeric extended modifier-key encoding is enabled.
    #[must_use]
    pub fn modify_other_keys_enabled(&self) -> bool {
        self.flags.modify_other_keys_2
    }
}
