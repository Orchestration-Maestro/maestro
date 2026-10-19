//! Shared row walk and plain-text cell emission.

use super::{PageFormatter, TrailingState};
use crate::page::{Cell, CellPosition, cell::Wide};
use std::fmt::{self, Write};

impl PageFormatter<'_> {
    /// Write text and return blanks deferred beyond the last emitted cell.
    ///
    /// Prior state is used only when both starting coordinates are zero.
    /// Out-of-range or inverted bounds emit nothing.
    ///
    /// # Errors
    /// Returns the destination's formatting error.
    pub fn format_with_state(&self, writer: &mut impl Write) -> Result<TrailingState, fmt::Error> {
        let mut state = if self.start_x == 0 && self.start_y == 0 {
            self.trailing_state.unwrap_or_default()
        } else {
            TrailingState::default()
        };
        let Some((end_x, end_y)) = self.bounds() else {
            return Ok(state);
        };
        for y in self.start_y..=end_y {
            self.write_row(writer, y, (end_x, end_y), &mut state)?;
        }
        Ok(state)
    }

    /// Clamp inclusive bounds and include the wrapped wide character at an end spacer.
    fn bounds(&self) -> Option<(u16, u16)> {
        let size = self.page.size;
        if self.start_x >= size.cols || self.start_y >= size.rows {
            return None;
        }
        let mut end_x = self.end_x.unwrap_or(size.cols - 1).min(size.cols - 1);
        let mut end_y = self.end_y.unwrap_or(size.rows - 1).min(size.rows - 1);
        if self.start_y > end_y {
            return None;
        }
        if self.opts.unwrap
            && !self.rectangle
            && end_y < size.rows - 1
            && self.page.get_cells(end_y)[usize::from(end_x)].wide() == Wide::SpacerHead
        {
            end_y += 1;
            end_x = 0;
        }
        if self.start_y == end_y && self.start_x > end_x {
            return None;
        }
        Some((end_x, end_y))
    }

    /// Select columns, moving a wide tail back to its leading cell.
    fn columns(&self, y: u16, end: (u16, u16)) -> Option<std::ops::Range<u16>> {
        let stop = if self.rectangle || y == end.1 {
            end.0 + 1
        } else {
            self.page.size.cols
        };
        let mut start = if self.rectangle || y == self.start_y {
            self.start_x
        } else {
            0
        };
        if start > 0 {
            match self.page.get_cells(y)[usize::from(start)].wide() {
                Wide::SpacerTail => start -= 1,
                Wide::SpacerHead => return None,
                Wide::Narrow | Wide::Wide => {}
            }
        }
        (start <= stop).then_some(start..stop)
    }

    /// Emit a nonempty row, retaining only blanks needed by subsequent text.
    fn write_row(
        &self,
        writer: &mut impl Write,
        y: u16,
        end: (u16, u16),
        state: &mut TrailingState,
    ) -> fmt::Result {
        let Some(columns) = self.columns(y, end) else {
            return Ok(());
        };
        let cells = self.page.get_cells(y);
        if !Cell::has_text_any(&cells[usize::from(columns.start)..usize::from(columns.end)]) {
            state.rows += 1;
            return Ok(());
        }
        for _ in 0..state.rows {
            writer.write_char('\n')?;
        }
        let row = self.page.get_row(y);
        state.rows = usize::from(!row.wrap() || !self.opts.unwrap);
        if !row.wrap_continuation() || !self.opts.unwrap {
            state.cells = 0;
        }
        for x in columns {
            self.write_cell(writer, CellPosition { x, y }, state)?;
        }
        Ok(())
    }

    /// Apply the last matching substitution to one scalar, including cluster suffixes.
    fn write_codepoint(&self, writer: &mut impl Write, cp: char) -> fmt::Result {
        match self
            .opts
            .codepoint_map
            .iter()
            .rev()
            .find(|map| map.range.contains(&cp))
        {
            Some(map) => match map.replacement {
                super::Replacement::Codepoint(replacement) => writer.write_char(replacement),
                super::Replacement::String(replacement) => writer.write_str(replacement),
            },
            None => writer.write_char(cp),
        }
    }

    /// Emit a cell's UTF-8 cluster after any preceding blank cells.
    fn write_cell(
        &self,
        writer: &mut impl Write,
        position: CellPosition,
        state: &mut TrailingState,
    ) -> fmt::Result {
        let cell = self.page.get_cells(position.y)[usize::from(position.x)];
        if matches!(cell.wide(), Wide::SpacerHead | Wide::SpacerTail) {
            return Ok(());
        }
        if !cell.has_text() || (cell.codepoint() == ' ' && self.opts.trim) {
            state.cells += 1;
            return Ok(());
        }
        for _ in 0..state.cells {
            writer.write_char(' ')?;
        }
        state.cells = 0;
        self.write_codepoint(writer, cell.codepoint())?;
        if let Some(suffix) = self.page.lookup_grapheme(position) {
            for cp in suffix {
                self.write_codepoint(writer, *cp)?;
            }
        }
        Ok(())
    }
}
