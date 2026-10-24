//! Line and display erasure through screen-owned clearing.

use super::Terminal;
use crate::{
    ansi::ProtectedMode,
    csi::{EraseDisplay, EraseLine},
    page::cell::Wide,
};

impl Terminal {
    /// Erase a display region, preserving a final primary prompt in history.
    pub fn erase_display(&mut self, mode: EraseDisplay, protected: bool) {
        let selective = protected || self.screens.active().protected_mode == ProtectedMode::Iso;
        match mode {
            EraseDisplay::ScrollComplete => {
                let screen = self.screens.active_mut();
                screen.scroll_clear();
                screen.cursor.pending_wrap = false;
                self.erase_images();
            }
            EraseDisplay::Complete => self.erase_complete(selective),
            EraseDisplay::Below => {
                self.erase_line(EraseLine::Right, protected);
                let screen = self.screens.active_mut();
                let y = screen.cursor.y + 1;
                if y < screen.pages.rows() {
                    screen.clear_rows(
                        crate::Point::Active(crate::Coordinate {
                            x: 0,
                            y: u32::from(y),
                        }),
                        None,
                        selective,
                    );
                }
            }
            EraseDisplay::Above => {
                self.erase_line(EraseLine::Left, protected);
                let screen = self.screens.active_mut();
                if screen.cursor.y > 0 {
                    let bottom = crate::Point::Active(crate::Coordinate {
                        x: 0,
                        y: u32::from(screen.cursor.y - 1),
                    });
                    screen.clear_rows(
                        crate::Point::Active(crate::Coordinate::default()),
                        Some(bottom),
                        selective,
                    );
                }
            }
            EraseDisplay::Scrollback => self.screens.active_mut().erase_history(None),
        }
    }

    /// Erase physical image placements and their now-unused image payloads.
    fn erase_images(&mut self) {
        let screen = self.screens.active_mut();
        screen.kitty_images.delete(
            &mut screen.pages,
            (0, 0),
            crate::Coordinate::default(),
            crate::kitty::graphics::command::Delete::All(true),
        );
    }

    /// Preserve primary prompt history before clearing the entire active area.
    fn erase_complete(&mut self, selective: bool) {
        let screen = self.screens.active_mut();
        let at_prompt = screen
            .pages
            .last_nonempty_active_row()
            .and_then(|(pin, _)| pin.row_and_cell(&screen.pages))
            .is_some_and(|(row, _)| {
                row.semantic_prompt() != crate::page::row::SemanticPrompt::None
            });
        if self.screens.active_key() == crate::screens::Key::Primary && at_prompt {
            self.screens.active_mut().scroll_clear();
        }
        let screen = self.screens.active_mut();
        screen.clear_rows(
            crate::Point::Active(crate::Coordinate::default()),
            None,
            selective,
        );
        screen.cursor.pending_wrap = false;
        self.erase_images();
        self.flags.dirty.clear = true;
    }

    /// Erase the selected line range, respecting ISO or explicitly requested protection.
    pub fn erase_line(&mut self, mode: EraseLine, protected: bool) {
        let cols = self.cols();
        let screen = self.screens.active_mut();
        if matches!(mode, EraseLine::Unknown(_))
            || (mode == EraseLine::RightUnlessPendingWrap && screen.cursor.pending_wrap)
        {
            return;
        }
        let Some(pin) = screen.pages.tracked_pin(screen.cursor.page_pin).copied() else {
            return;
        };
        let Some((_, cell)) = pin.row_and_cell(&screen.pages) else {
            return;
        };
        let wide = cell.wide();
        let x = screen.cursor.x;
        let columns = match mode {
            EraseLine::Right | EraseLine::RightUnlessPendingWrap => {
                screen.cursor_reset_wrap();
                x - u16::from(x > 0 && wide == Wide::SpacerTail)..cols
            }
            EraseLine::Left => 0..x + 1 + u16::from(wide == Wide::Wide),
            EraseLine::Complete => 0..cols,
            EraseLine::Unknown(_) => return,
        };
        screen.cursor.pending_wrap = false;
        screen.cursor_mark_dirty();
        if protected || screen.protected_mode == ProtectedMode::Iso {
            screen.clear_unprotected_cells(pin.node, pin.y, columns);
        } else {
            screen.clear_cells(pin.node, pin.y, columns);
        }
    }
}
