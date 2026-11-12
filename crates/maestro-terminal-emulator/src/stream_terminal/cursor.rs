//! Cursor defaults, one-axis origin coordinates and bounded tab traversal.

use super::Handler;
use crate::{Mode, ansi::CursorStyle as Request, cursor::Style};

impl Handler<'_> {
    /// Retain the untouched axis in the coordinate space accepted by the terminal.
    pub(super) fn axis(&mut self, col: Option<usize>, row: Option<usize>) {
        let t = &mut *self.terminal;
        let cursor = &t.screens.active().cursor;
        let (left, top) = if t.modes.get(Mode::Origin) {
            (t.scrolling_region.left, t.scrolling_region.top)
        } else {
            (0, 0)
        };
        let x = usize::from(cursor.x.saturating_sub(left)) + 1;
        let y = usize::from(cursor.y.saturating_sub(top)) + 1;
        t.set_cursor_pos(row.unwrap_or(y), col.unwrap_or(x));
    }

    /// Add relative movement in origin-relative coordinates without moving the other axis.
    pub(super) fn relative_axis(&mut self, count: u16, vertical: bool) {
        let t = &self.terminal;
        let cursor = &t.screens.active().cursor;
        let origin = t.modes.get(Mode::Origin);
        let (value, offset) = if vertical {
            (cursor.y, if origin { t.scrolling_region.top } else { 0 })
        } else {
            (cursor.x, if origin { t.scrolling_region.left } else { 0 })
        };
        let next =
            (usize::from(value) + usize::from(count) + 1).saturating_sub(usize::from(offset));
        if vertical {
            self.axis(None, Some(next));
        } else {
            self.axis(Some(next), None);
        }
    }

    /// Stop repeated tabs when the existing owner can no longer advance.
    pub(super) fn tabs(&mut self, count: u16, backward: bool) {
        for _ in 0..count {
            let x = self.terminal.screens.active().cursor.x;
            if backward {
                self.terminal.horizontal_tab_back();
            } else {
                self.terminal.horizontal_tab();
            }
            if x == self.terminal.screens.active().cursor.x {
                break;
            }
        }
    }

    /// Select explicit cursor appearance or restore retained defaults.
    pub(super) fn cursor_style(&mut self, request: Request) {
        self.default_cursor = request == Request::Default;
        let blink = match request {
            Request::Default => self.default_cursor_blink,
            Request::BlinkingBlock | Request::BlinkingBar | Request::BlinkingUnderline => true,
            Request::SteadyBlock | Request::SteadyBar | Request::SteadyUnderline => false,
        };
        let style = match request {
            Request::Default => self.default_cursor_style,
            Request::BlinkingBlock | Request::SteadyBlock => Style::Block,
            Request::BlinkingBar | Request::SteadyBar => Style::Bar,
            Request::BlinkingUnderline | Request::SteadyUnderline => Style::Underline,
        };
        self.terminal.modes.set(Mode::CursorBlinking, blink);
        self.terminal.screens.active_mut().cursor.cursor_style = style;
    }
}
