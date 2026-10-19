//! Cluster continuation and width changes on the cursor row.

use super::super::Terminal;
use crate::{
    Mode,
    page::{CellPosition, cell::Wide},
    page_list::IncreaseCapacityError as Error,
    unicode::grapheme::{self, BreakState, GraphemeWidthEffect},
};

impl Terminal {
    /// Resolve the base column before the cursor, accounting for a pending wrap and tail.
    pub(super) fn previous_column(&self, right: u16, clustered: bool) -> Option<u16> {
        let screen = self.screens.active();
        let c = &screen.cursor;
        let pin = screen.pages.tracked_pin(c.page_pin)?;
        let page = screen.pages.page(pin.node)?;
        let under = page.get_cells(pin.y)[usize::from(c.x)];
        let left = if self.modes.get(Mode::Wraparound) {
            u16::from(!c.pending_wrap)
        } else if clustered && c.x == right - 1 {
            u16::from(under.codepoint() == '\0')
        } else {
            1
        };
        let x = c.x.checked_sub(left)?;
        if page.get_cells(pin.y)[usize::from(x)].wide() == Wide::SpacerTail {
            x.checked_sub(1)
        } else {
            Some(x)
        }
    }

    /// Append a scalar only if the existing cluster has no boundary before it.
    pub(super) fn continue_cluster(&mut self, cp: char, right: u16) -> Result<bool, Error> {
        if cp <= '\u{ff}'
            || !self.modes.get(Mode::GraphemeCluster)
            || self.screens.active().cursor.x == 0
        {
            return Ok(false);
        }
        let Some(mut x) = self.previous_column(right, true) else {
            return Ok(false);
        };
        let screen = self.screens.active();
        let pin = *screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .ok_or(Error::InvalidNode)?;
        let page = screen.pages.page(pin.node).ok_or(Error::InvalidNode)?;
        let cell = page.get_cells(pin.y)[usize::from(x)];
        let mut previous = cell.codepoint();
        if previous == '\0' {
            return Ok(false);
        }
        let mut state = BreakState::default();
        if let Some(suffix) = page.lookup_grapheme(CellPosition { x, y: pin.y }) {
            for &scalar in suffix {
                grapheme::grapheme_break(previous, scalar, &mut state);
                previous = scalar;
            }
        }
        if grapheme::grapheme_break(previous, cp, &mut state) {
            return Ok(false);
        }
        match grapheme::grapheme_width_effect(previous, cp) {
            GraphemeWidthEffect::Ignore => return Ok(true),
            GraphemeWidthEffect::Wide if cell.wide() != Wide::Wide => {
                let Some(column) = self.widen_cluster(x, right)? else {
                    return Ok(true);
                };
                x = column;
            }
            GraphemeWidthEffect::Narrow if cell.wide() == Wide::Wide => {
                self.narrow_cluster(x, right)?;
            }
            _ => {}
        }
        let screen = self.screens.active_mut();
        screen.cursor_mark_dirty();
        screen.append_grapheme(x, cp)?;
        Ok(true)
    }

    /// Expand a cluster, moving its owned suffix across a wrap when necessary.
    fn widen_cluster(&mut self, mut x: u16, right: u16) -> Result<Option<u16>, Error> {
        self.screens.active_mut().cursor_horizontal_absolute(x);
        if x == right - 1 {
            if !self.modes.get(Mode::Wraparound) {
                return Ok(None);
            }
            let screen = self.screens.active_mut();
            let pin = *screen
                .pages
                .tracked_pin(screen.cursor.page_pin)
                .ok_or(Error::InvalidNode)?;
            let page = screen.pages.page_mut(pin.node).ok_or(Error::InvalidNode)?;
            let cp = page.get_cells(pin.y)[usize::from(x)].codepoint();
            let suffix = page.take_grapheme(CellPosition { x, y: pin.y });
            page.update_row_grapheme_flag(pin.y);
            let row_wrap = right == self.cols;
            if row_wrap {
                page.get_row_mut(pin.y).set_wrap(true);
            }
            self.print_cell(
                '\0',
                if row_wrap {
                    Wide::SpacerHead
                } else {
                    Wide::Narrow
                },
            )?;
            self.print_wrap()?;
            self.print_cell(cp, Wide::Wide)?;
            x = self.screens.active().cursor.x;
            for scalar in suffix.into_iter().flatten() {
                self.screens.active_mut().append_grapheme(x, scalar)?;
            }
        } else {
            self.cluster_cell_wide(x, Wide::Wide)?;
        }
        self.screens.active_mut().cursor_right(1);
        self.print_cell('\0', Wide::SpacerTail)?;
        let screen = self.screens.active_mut();
        if screen.cursor.x == right - 1 {
            screen.cursor.pending_wrap = true;
        } else {
            screen.cursor_right(1);
        }
        Ok(Some(x))
    }

    /// Remove the tail role and compensate the cursor after VS15.
    fn narrow_cluster(&mut self, x: u16, right: u16) -> Result<(), Error> {
        self.cluster_cell_wide(x, Wide::Narrow)?;
        self.cluster_cell_wide(x + 1, Wide::Narrow)?;
        let screen = self.screens.active_mut();
        if screen.cursor.x == right - 1 {
            screen.cursor.pending_wrap = false;
        } else {
            screen.cursor_left(1);
        }
        Ok(())
    }

    /// Change only a cell's width role, leaving managed data intact.
    fn cluster_cell_wide(&mut self, x: u16, wide: Wide) -> Result<(), Error> {
        let screen = self.screens.active_mut();
        let pin = screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .copied()
            .ok_or(Error::InvalidNode)?;
        screen
            .pages
            .page_mut(pin.node)
            .ok_or(Error::InvalidNode)?
            .get_cells_mut(pin.y)[usize::from(x)]
        .set_wide(wide);
        Ok(())
    }
}
