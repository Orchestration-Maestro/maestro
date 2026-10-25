//! Mode bits and their terminal-owned side effects.

use super::Handler;
use crate::{
    Mode,
    mouse::{Event, Format},
    terminal::{DeccolmMode, SwitchScreenMode},
};

impl Handler<'_> {
    /// Commit the mode before applying its best-effort side effects.
    pub(super) fn set_mode(&mut self, mode: Mode, enabled: bool) {
        let t = &mut *self.terminal;
        t.modes.set(mode, enabled);
        match mode {
            Mode::Origin => t.set_cursor_pos(1, 1),
            Mode::EnableLeftAndRightMargin if !enabled => {
                t.scrolling_region.left = 0;
                t.scrolling_region.right = t.cols() - 1;
            }
            Mode::AltScreenLegacy => {
                let _ = t.switch_screen_mode(SwitchScreenMode::Mode47, enabled);
            }
            Mode::AltScreen => {
                let _ = t.switch_screen_mode(SwitchScreenMode::Mode1047, enabled);
            }
            Mode::AltScreenSaveCursorClearEnter => {
                let _ = t.switch_screen_mode(SwitchScreenMode::Mode1049, enabled);
            }
            Mode::SaveCursor => {
                if enabled {
                    t.save_cursor();
                } else {
                    t.restore_cursor();
                }
            }
            Mode::Column132 => {
                let _ = t.deccolm(if enabled {
                    DeccolmMode::Cols132
                } else {
                    DeccolmMode::Cols80
                });
            }
            Mode::MouseEventX10 => {
                t.flags.mouse_event = if enabled { Event::X10 } else { Event::None }
            }
            Mode::MouseEventNormal => {
                t.flags.mouse_event = if enabled { Event::Normal } else { Event::None }
            }
            Mode::MouseEventButton => {
                t.flags.mouse_event = if enabled { Event::Button } else { Event::None }
            }
            Mode::MouseEventAny => {
                t.flags.mouse_event = if enabled { Event::Any } else { Event::None }
            }
            Mode::MouseFormatUtf8 => self.mouse_format(Format::Utf8, enabled),
            Mode::MouseFormatSgr => self.mouse_format(Format::Sgr, enabled),
            Mode::MouseFormatUrxvt => self.mouse_format(Format::Urxvt, enabled),
            Mode::MouseFormatSgrPixels => self.mouse_format(Format::SgrPixels, enabled),
            _ => {}
        }
    }
}

impl Handler<'_> {
    /// Disable any selected encoding back to the legacy format.
    fn mouse_format(&mut self, format: Format, enabled: bool) {
        self.terminal.flags.mouse_format = if enabled { format } else { Format::X10 };
    }
}
