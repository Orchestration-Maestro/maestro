//! Terminal screen switching and protocol-mode ordering.

use super::{Flags, ResizeError, ScrollingRegion, TABSTOP_INTERVAL, Terminal};
use crate::{Mode, page_list::IncreaseCapacityError, screen::Options, screen_set::Key};
use std::num::NonZeroU16;

/// Alternate-screen protocol modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchScreenMode {
    /// Preserve both screens and copy the cursor on a real switch.
    Mode47,
    /// Clear alternate content on exit and copy the cursor on a real switch.
    Mode1047,
    /// Save on every entry, clear alternate content, and restore primary on exit.
    Mode1049,
}

impl Terminal {
    /// Select a screen, returning the previous identity only when it changes.
    pub fn switch_screen(&mut self, key: Key) -> Option<Key> {
        let old = self.screens.active_key();
        if old == key {
            return None;
        }
        self.screens.active_mut().end_hyperlink();
        let charset = self.screens.active().charset;
        let opts = Options {
            cols: NonZeroU16::new(self.cols).unwrap_or(NonZeroU16::MIN),
            rows: NonZeroU16::new(self.rows).unwrap_or(NonZeroU16::MIN),
            max_scrollback: if key == Key::Primary {
                self.max_scrollback
            } else {
                0
            },
        };
        if self.screens.get(key).is_none() {
            let primary = self.screens.get(Key::Primary)?;
            let (limit, permissions) = (
                primary.kitty_images.total_limit,
                primary.kitty_images.image_limits,
            );
            let new = self.screens.get_init(key, opts);
            new.kitty_images.total_limit = limit;
            new.kitty_images.image_limits = permissions;
        }
        let new = self.screens.get_init(key, opts);
        new.charset = charset;
        new.kitty_images.dirty = true;
        // Initialization above guarantees that the destination is resident.
        let _ = self.screens.switch_to(key);
        self.flags.dirty.clear = true;
        Some(old)
    }

    /// Apply alternate-screen cursor-save, clear and copy ordering.
    ///
    /// # Errors
    /// Returns a capacity refusal if the copied cursor style cannot be retained.
    pub fn switch_screen_mode(
        &mut self,
        mode: SwitchScreenMode,
        enabled: bool,
    ) -> Result<(), IncreaseCapacityError> {
        if mode == SwitchScreenMode::Mode1047
            && !enabled
            && self.screens.active_key() == Key::Alternate
        {
            self.erase_display(crate::csi::EraseDisplay::Complete, false);
        }
        if mode == SwitchScreenMode::Mode1049 && enabled {
            self.save_cursor();
        }
        let old = self.switch_screen(if enabled {
            Key::Alternate
        } else {
            Key::Primary
        });
        if mode == SwitchScreenMode::Mode1049 {
            if enabled {
                self.erase_display(crate::csi::EraseDisplay::Complete, false);
            } else {
                self.restore_cursor();
                return Ok(());
            }
        }
        if let Some(old) = old
            && let Some((source, destination)) = self.screens.pair_mut(old)
        {
            destination.cursor_copy(&source.cursor, false)?;
        }
        Ok(())
    }
}

/// Protocol-selected grid width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeccolmMode {
    /// Eighty columns.
    Cols80,
    /// One hundred thirty-two columns.
    Cols132,
}

impl Terminal {
    /// Select a fixed protocol width only when mode 40 permits it, clearing the display.
    ///
    /// # Errors
    /// Returns the delegated screen resize refusal without rolling back earlier changes.
    pub fn deccolm(&mut self, mode: DeccolmMode) -> Result<(), ResizeError> {
        if !self.modes.get(Mode::EnableMode3) {
            self.modes.set(Mode::Column132, false);
            return Ok(());
        }
        self.modes
            .set(Mode::Column132, mode == DeccolmMode::Cols132);
        self.resize_grid(
            if mode == DeccolmMode::Cols132 {
                132
            } else {
                80
            },
            self.rows,
        )?;
        self.erase_display(crate::csi::EraseDisplay::Complete, false);
        self.set_cursor_pos(1, 1);
        Ok(())
    }
}

impl Terminal {
    /// Clear screen and protocol state while retaining geometry, colors and mode defaults.
    pub fn full_reset(&mut self) {
        // Deselect alternate storage before removing its owned lifetime.
        let _ = self.screens.switch_to(Key::Primary);
        let _ = self.screens.remove(Key::Alternate);
        self.screens.active_mut().reset();
        self.modes.reset();
        self.flags = Flags::default();
        self.tabstops.reset(TABSTOP_INTERVAL);
        self.previous_char = None;
        self.pwd.clear();
        self.title.clear();
        self.status_display = crate::ansi::StatusDisplay::Main;
        self.scrolling_region = ScrollingRegion {
            top: 0,
            bottom: self.rows - 1,
            left: 0,
            right: self.cols - 1,
        };
        self.flags.dirty.clear = true;
    }
}
