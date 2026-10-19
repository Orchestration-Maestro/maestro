//! Scalar printing shared by all text entry points.

mod cells;
mod clusters;

use super::Terminal;
use crate::{
    Mode,
    ansi::StatusDisplay,
    charsets::{ActiveSlot, Charset, Slots},
    page::{
        cell::{SemanticContent, Wide},
        row::SemanticPrompt,
    },
    page_list::IncreaseCapacityError as Error,
    unicode::{
        self,
        props::{GraphemeBreak, Properties},
    },
};

impl Terminal {
    /// Print UTF-8 text, interpreting newline as carriage return followed by linefeed.
    ///
    /// # Errors
    /// Returns a capacity error when text or scrolling cannot fit managed storage.
    pub fn print_string(&mut self, text: &str) -> Result<(), Error> {
        for cp in text.chars() {
            if cp == '\n' {
                self.carriage_return();
                self.linefeed()?;
            } else {
                self.print(cp)?;
            }
        }
        Ok(())
    }

    /// Print an already decoded sequence of printable scalars.
    ///
    /// Control-sequence parsing belongs to the caller.
    ///
    /// # Errors
    /// Returns a capacity error when text or scrolling cannot fit managed storage.
    pub fn print_slice(&mut self, scalars: &[char]) -> Result<(), Error> {
        for &cp in scalars {
            self.print(cp)?;
        }
        Ok(())
    }

    /// Repeat the last printable scalar at least once; do nothing if none exists.
    ///
    /// # Errors
    /// Returns a capacity error when text or scrolling cannot fit managed storage.
    pub fn print_repeat(&mut self, count: usize) -> Result<(), Error> {
        if let Some(cp) = self.previous_char {
            for _ in 0..count.max(1) {
                self.print(cp)?;
            }
        }
        Ok(())
    }

    /// Print one scalar through charset mapping, clustering and pending wrap.
    ///
    /// Control-sequence parsing belongs to the caller.
    ///
    /// # Errors
    /// Returns a capacity error when text or scrolling cannot fit managed storage.
    pub fn print(&mut self, cp: char) -> Result<(), Error> {
        if self.status_display != StatusDisplay::Main {
            return Ok(());
        }
        let right = if self.screens.active().cursor.x > self.scrolling_region.right {
            self.cols
        } else {
            self.scrolling_region.right + 1
        };
        if self.continue_cluster(cp, right)? {
            return Ok(());
        }
        let width = if cp <= '\u{ff}' {
            1
        } else {
            unicode::codepoint_width(cp)
        };
        if width == 0 {
            return self.print_zero_width(cp, right);
        }
        self.previous_char = Some(cp);
        if self.screens.active().cursor.pending_wrap && self.modes.get(Mode::Wraparound) {
            self.print_wrap()?;
        }
        if self.modes.get(Mode::Insert)
            && u32::from(self.screens.active().cursor.x) + u32::from(width) < u32::from(self.cols)
        {
            self.insert_blanks(usize::from(width));
        }
        if width == 1 {
            self.screens.active_mut().cursor_mark_dirty();
            self.print_cell(cp, Wide::Narrow, self.screens.active().cursor.protected)?;
        } else if !self.print_wide(cp, right)? {
            return Ok(());
        }
        let screen = self.screens.active_mut();
        if screen.cursor.x == right - 1 {
            screen.cursor.pending_wrap = true;
        } else {
            screen.cursor_right(1);
        }
        Ok(())
    }

    /// Attach a zero-width scalar without clustering when the mode is disabled.
    fn print_zero_width(&mut self, cp: char, right: u16) -> Result<(), Error> {
        if self.modes.get(Mode::GraphemeCluster) {
            return Ok(());
        }
        let Some(x) = self.previous_column(right, false) else {
            return Ok(());
        };
        let screen = self.screens.active();
        let pin = screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .ok_or(Error::InvalidNode)?;
        let cell = screen
            .pages
            .page(pin.node)
            .ok_or(Error::InvalidNode)?
            .get_cells(pin.y)[usize::from(x)];
        if !cell.has_text() {
            return Ok(());
        }
        if matches!(cp, '\u{fe0e}' | '\u{fe0f}')
            && Properties::get(cell.codepoint()).grapheme_break
                != GraphemeBreak::ExtendedPictographic
        {
            return Ok(());
        }
        self.screens.active_mut().append_grapheme(x, cp)
    }

    /// Write a wide scalar and its tail, inserting a head spacer before a wrap.
    fn print_wide(&mut self, cp: char, right: u16) -> Result<bool, Error> {
        if right - self.scrolling_region.left <= 1 {
            self.screens.active_mut().cursor_mark_dirty();
            self.print_cell('\0', Wide::Narrow, self.screens.active().cursor.protected)?;
            return Ok(true);
        }
        if self.screens.active().cursor.x == right - 1 {
            if !self.modes.get(Mode::Wraparound) {
                return Ok(false);
            }
            if right == self.cols {
                self.set_cursor_row_wrap(true)?;
            }
            self.print_cell(
                '\0',
                if right == self.cols {
                    Wide::SpacerHead
                } else {
                    Wide::Narrow
                },
                self.screens.active().cursor.protected,
            )?;
            self.print_wrap()?;
        }
        self.screens.active_mut().cursor_mark_dirty();
        self.print_cell(cp, Wide::Wide, self.screens.active().cursor.protected)?;
        self.screens.active_mut().cursor_right(1);
        self.print_cell(
            '\0',
            Wide::SpacerTail,
            self.screens.active().cursor.protected,
        )?;
        Ok(true)
    }

    /// Continue a soft-wrapped row, preserving semantic prompt state across indexing.
    fn print_wrap(&mut self) -> Result<(), Error> {
        let c = &self.screens.active().cursor;
        let mark = c.x == self.cols - 1;
        let (semantic, clear) = (c.semantic_content, c.semantic_content_clear_eol);
        if mark {
            self.set_cursor_row_wrap(true)?;
        }
        self.index()?;
        let screen = self.screens.active_mut();
        screen.cursor_horizontal_absolute(self.scrolling_region.left);
        screen.cursor.semantic_content = semantic;
        screen.cursor.semantic_content_clear_eol = clear;
        let pin = screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .copied()
            .ok_or(Error::InvalidNode)?;
        let (row, _) = pin
            .row_and_cell_mut(&mut screen.pages)
            .ok_or(Error::InvalidNode)?;
        if semantic == SemanticContent::Prompt {
            row.set_semantic_prompt(SemanticPrompt::PromptContinuation);
        }
        if mark {
            row.set_wrap_continuation(true);
        }
        Ok(())
    }

    /// Set the current row's wrap metadata without dirtying unrelated rows.
    fn set_cursor_row_wrap(&mut self, wrap: bool) -> Result<(), Error> {
        let screen = self.screens.active_mut();
        let pin = screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .copied()
            .ok_or(Error::InvalidNode)?;
        pin.row_and_cell_mut(&mut screen.pages)
            .ok_or(Error::InvalidNode)?
            .0
            .set_wrap(wrap);
        Ok(())
    }

    /// Configure one charset slot on the active screen.
    pub fn configure_charset(&mut self, slot: Slots, set: Charset) {
        self.screens.active_mut().charset.charsets.set(slot, set);
    }

    /// Invoke a charset permanently, or for one GL cell write.
    ///
    /// # Panics
    /// Panics if a single shift is requested for GR rather than GL.
    pub fn invoke_charset(&mut self, active: ActiveSlot, slot: Slots, single: bool) {
        let state = &mut self.screens.active_mut().charset;
        if single {
            assert_eq!(active, ActiveSlot::Gl);
            state.single_shift = Some(slot);
        } else {
            match active {
                ActiveSlot::Gl => state.gl = slot,
                ActiveSlot::Gr => state.gr = slot,
            }
        }
    }
}
