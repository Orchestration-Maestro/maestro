//! Screen alignment with retained colours and unprotected narrow cells.

use super::{ScrollingRegion, Terminal};
use crate::{Coordinate, Mode, Point, page::Cell, page_list::IncreaseCapacityError, style::Style};

impl Terminal {
    /// Fill the active grid with E, resetting margins and non-colour attributes.
    ///
    /// # Errors
    /// Returns the existing page-capacity refusal if the colour-only style cannot be retained.
    pub fn decaln(&mut self) -> Result<(), IncreaseCapacityError> {
        let screen = self.screens.active_mut();
        let old = screen.cursor.style;
        screen.cursor.style = Style {
            fg_color: old.fg_color,
            bg_color: old.bg_color,
            ..Style::default()
        };
        if let Err(error) = screen.manual_style_update() {
            screen.cursor.style = old;
            return Err(error);
        }
        self.scrolling_region = ScrollingRegion {
            top: 0,
            bottom: self.rows() - 1,
            left: 0,
            right: self.cols() - 1,
        };
        self.modes.set(Mode::Origin, false);
        self.set_cursor_pos(1, 1);
        let screen = self.screens.active_mut();
        screen.clear_rows(Point::Active(Coordinate::default()), None, false);
        loop {
            let pin = screen
                .pages
                .tracked_pin(screen.cursor.page_pin)
                .copied()
                .ok_or(IncreaseCapacityError::InvalidNode)?;
            let page = screen
                .pages
                .page_mut(pin.node)
                .ok_or(IncreaseCapacityError::InvalidNode)?;
            let mut cell = Cell::init('E');
            cell.set_style_id(screen.cursor.style_id);
            page.get_cells_mut(pin.y).fill(cell);
            #[cfg(test)]
            {
                crate::tests::WORK
                    .with_borrow_mut(|work| work.row_shift += usize::from(page.size.cols));
            }
            if screen.cursor.style_id != 0 {
                page.styles
                    .use_multiple(screen.cursor.style_id, page.size.cols);
                page.get_row_mut(pin.y).set_styled(true);
            }
            screen.cursor_mark_dirty();
            if screen.cursor.y == screen.pages.rows() - 1 {
                break;
            }
            screen.cursor_down(1);
        }
        self.set_cursor_pos(1, 1);
        Ok(())
    }
}
