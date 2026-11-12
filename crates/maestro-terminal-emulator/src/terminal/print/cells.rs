//! Managed scalar writes and charset translation.

use super::super::Terminal;
use crate::{
    charsets::{self, Charset},
    page::{Cell, CellPosition, cell::Wide},
    page_list::IncreaseCapacityError as Error,
};

impl Terminal {
    /// Write one mapped scalar while preserving page-local managed ownership.
    pub(super) fn print_cell(
        &mut self,
        cp: char,
        wide: Wide,
        protected: bool,
    ) -> Result<(), Error> {
        let cp = self.map_charset(cp);
        self.clear_overwritten_wide(wide)?;
        let screen = self.screens.active_mut();
        let pin = screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .copied()
            .ok_or(Error::InvalidNode)?;
        let position = CellPosition { x: pin.x, y: pin.y };
        let page = screen.pages.page_mut(pin.node).ok_or(Error::InvalidNode)?;
        let old = page.get_cells(pin.y)[usize::from(pin.x)];
        if old.has_grapheme() {
            page.clear_grapheme(position);
            page.update_row_grapheme_flag(pin.y);
        }
        if old.style_id() != screen.cursor.style_id {
            if old.style_id() != 0 {
                page.styles.release_multiple(old.style_id(), 1);
            }
            if screen.cursor.style_id != 0 {
                page.styles.use_multiple(screen.cursor.style_id, 1);
            }
        }
        if old.hyperlink() {
            page.clear_hyperlink(position);
        }
        let mut cell = Cell::init(cp);
        cell.set_wide(wide);
        cell.set_style_id(screen.cursor.style_id);
        cell.set_protected(protected);
        cell.set_semantic_content(screen.cursor.semantic_content);
        page.get_cells_mut(pin.y)[usize::from(pin.x)] = cell;
        if cell.has_styling() {
            page.get_row_mut(pin.y).set_styled(true);
        }
        if cp == '\u{10eeee}' {
            page.get_row_mut(pin.y).set_kitty_virtual_placeholder(true);
        }
        if screen.cursor.hyperlink_id != 0 {
            screen.cursor_set_hyperlink()?;
        } else if old.hyperlink() {
            page.update_row_hyperlink_flag(pin.y);
        }
        Ok(())
    }

    /// Consume a single shift even for a spacer write, matching charset invocation.
    fn map_charset(&mut self, cp: char) -> char {
        let state = &mut self.screens.active_mut().charset;
        let slot = state.single_shift.take().unwrap_or(state.gl);
        let set = state.charsets.get(slot);
        if matches!(set, Charset::Ascii | Charset::Utf8) {
            return cp;
        }
        u8::try_from(u32::from(cp))
            .ok()
            .and_then(|b| charsets::table(set).map(|t| t[usize::from(b)]))
            .and_then(|value| char::from_u32(u32::from(value)))
            .unwrap_or(' ')
    }

    /// Clear the detached halves of a wide character being overwritten.
    fn clear_overwritten_wide(&mut self, incoming: Wide) -> Result<(), Error> {
        let screen = self.screens.active_mut();
        let pin = screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .copied()
            .ok_or(Error::InvalidNode)?;
        let old = pin
            .row_and_cell(&screen.pages)
            .ok_or(Error::InvalidNode)?
            .1
            .wide();
        if old == incoming {
            return Ok(());
        }
        match old {
            Wide::Wide if screen.cursor.x < self.cols - 1 => {
                screen.clear_cells(pin.node, pin.y, pin.x + 1..pin.x + 2);
            }
            Wide::SpacerTail => screen.clear_cells(pin.node, pin.y, pin.x - 1..pin.x),
            _ => return Ok(()),
        }
        if screen.cursor.y > 0
            && screen.cursor.x <= 1
            && let Some(cell) = screen.cursor_cell_end_of_prev()
            && cell.wide() == Wide::SpacerHead
        {
            cell.set_wide(Wide::Narrow);
        }
        Ok(())
    }
}
