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

impl Terminal {
    /// Move to the left margin, or column zero when already left of it.
    pub fn carriage_return(&mut self) {
        let screen = self.screens.active_mut();
        screen.cursor.pending_wrap = false;
        let x = if self.modes.get(Mode::Origin) || screen.cursor.x >= self.scrolling_region.left {
            self.scrolling_region.left
        } else {
            0
        };
        screen.cursor_horizontal_absolute(x);
    }

    /// Move down, applying carriage return in newline mode.
    ///
    /// # Errors
    /// Returns a capacity error when scrolling cannot retain managed content.
    pub fn linefeed(&mut self) -> Result<(), crate::page_list::IncreaseCapacityError> {
        self.index()?;
        if self.modes.get(Mode::Linefeed) {
            self.carriage_return();
        }
        Ok(())
    }

    /// Move down within margins, scrolling at the bottom boundary.
    ///
    /// # Errors
    /// Returns a capacity error when scrolling cannot retain managed content.
    pub fn index(&mut self) -> Result<(), crate::page_list::IncreaseCapacityError> {
        let result = self.index_move();
        let screen = self.screens.active_mut();
        if screen.cursor.semantic_content != crate::page::cell::SemanticContent::Output {
            if screen.cursor.semantic_content_clear_eol {
                screen.cursor.semantic_content = crate::page::cell::SemanticContent::Output;
                screen.cursor.semantic_content_clear_eol = false;
            } else if let Some(pin) = screen.pages.tracked_pin(screen.cursor.page_pin).copied()
                && let Some((row, _)) = pin.row_and_cell_mut(&mut screen.pages)
            {
                row.set_semantic_prompt(crate::page::row::SemanticPrompt::PromptContinuation);
            }
        }
        result
    }

    /// Execute the movement portion before semantic prompt continuation is applied.
    fn index_move(&mut self) -> Result<(), crate::page_list::IncreaseCapacityError> {
        use crate::page_list::IncreaseCapacityError as Error;
        let r = self.scrolling_region;
        let screen = self.screens.active_mut();
        screen.cursor.pending_wrap = false;
        let (x, y) = (screen.cursor.x, screen.cursor.y);
        if y < r.top || y > r.bottom {
            if y < self.rows - 1 {
                screen.cursor_down(1);
            }
        } else if y == r.bottom && x >= r.left && x <= r.right {
            screen.kitty_images.dirty = true;
            if r.top == 0
                && r.left == 0
                && r.right == self.cols - 1
                && (!screen.no_scrollback || r.bottom == 0)
            {
                screen
                    .cursor_scroll_above()
                    .map_err(|_| Error::OutOfSpace)?;
            } else if r.left != 0 || r.right != self.cols - 1 {
                self.scroll_up(1)?;
            } else {
                screen
                    .cursor_scroll_region_up(usize::from(r.bottom - r.top))
                    .map_err(|_| Error::OutOfSpace)?;
            }
        } else if y < r.bottom {
            screen.cursor_down(1);
        }
        Ok(())
    }
}

impl Terminal {
    /// Move back one column using the configured reverse-wrap behavior.
    pub fn backspace(&mut self) {
        self.cursor_left(1);
    }

    /// Move upward, scrolling down when at the scrolling region's top edge.
    ///
    /// # Errors
    /// Returns a capacity error when a managed row cannot fit its destination.
    pub fn reverse_index(&mut self) -> Result<(), crate::page_list::IncreaseCapacityError> {
        let c = &self.screens.active().cursor;
        let r = self.scrolling_region;
        if c.y != r.top || c.x < r.left || c.x > r.right {
            self.cursor_up(1);
            Ok(())
        } else {
            self.scroll_down(1)
        }
    }
}
