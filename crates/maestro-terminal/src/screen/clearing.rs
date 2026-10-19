//! Background-preserving clearing and discontiguous wide-cell cleanup.

use std::ops::Range;

use super::Screen;
use crate::{
    Point,
    page::{Cell, cell::Wide, row::Row},
    page_list::{Direction, NodeId},
};

impl Screen {
    /// Construct an unstyled blank retaining only the cursor background.
    #[must_use]
    pub fn blank_cell(&self) -> Cell {
        self.cursor.style.bg_cell().unwrap_or_default()
    }

    /// Clear an inclusive region of rows, optionally retaining protected cells.
    pub fn clear_rows(&mut self, top: Point, bottom: Option<Point>, protected: bool) {
        let mut it = self.pages.page_iterator(Direction::RightDown, top, bottom);
        while let Some(chunk) = it.next(&self.pages) {
            for y in chunk.start..chunk.end {
                self.clear_row(chunk.node, y, protected);
            }
        }
    }

    /// Clear one stored row, retaining its metadata only for selective clearing.
    fn clear_row(&mut self, node: NodeId, y: u16, protected: bool) {
        let Some(cols) = self.pages.page(node).map(|page| page.size.cols) else {
            return;
        };
        if protected {
            self.clear_unprotected_cells(node, y, 0..cols);
        } else {
            self.clear_cells(node, y, 0..cols);
        }
        if let Some(page) = self.pages.page_mut(node) {
            let row = page.get_row_mut(y);
            if !protected {
                *row = Row::default();
            }
            row.set_dirty(true);
        }
    }

    /// Release managed values in a page-local cell range and apply blank cells.
    ///
    /// Stale page handles are ignored.
    ///
    /// # Panics
    /// Panics if a nonempty range lies outside the selected page row.
    pub fn clear_cells(&mut self, node: NodeId, y: u16, columns: Range<u16>) {
        if columns.is_empty() {
            return;
        }
        let blank = self.blank_cell();
        if let Some(page) = self.pages.page_mut(node) {
            page.clear_cells(y, columns.clone());
            page.get_cells_mut(y)[usize::from(columns.start)..usize::from(columns.end)].fill(blank);
        }
    }

    /// Clear consecutive unprotected runs, preserving protected managed owners.
    ///
    /// Stale page handles are ignored.
    ///
    /// # Panics
    /// Panics if the range lies outside the selected page row.
    pub fn clear_unprotected_cells(&mut self, node: NodeId, y: u16, columns: Range<u16>) {
        if columns.is_empty() {
            return;
        }
        let Some(page) = self.pages.page(node) else {
            return;
        };
        let cells = page.get_cells(y);
        let start = columns.start
            - u16::from(
                columns.start > 0 && cells[usize::from(columns.start)].wide() == Wide::SpacerTail,
            );
        let end = columns.end
            + u16::from(
                columns.end < page.size.cols
                    && cells[usize::from(columns.end - 1)].wide() == Wide::Wide,
            );
        let clear_previous = start == 0
            && page.get_row(y).wrap_continuation()
            && cells[0].wide() == Wide::Wide
            && !cells[0].protected();
        if clear_previous {
            let pin = crate::page_list::Pin {
                node,
                x: 0,
                y,
                garbage: false,
            };
            if let Some(previous) = pin.up(&self.pages, 1) {
                self.clear_spacer_head(previous);
            }
        }
        let blank = self.blank_cell();
        let mut x = start;
        while x < end {
            let Some(page) = self.pages.page(node) else {
                return;
            };
            let cells = page.get_cells(y);
            while x < end && cells[usize::from(x)].protected() {
                x += 1;
            }
            let start = x;
            while x < end && !cells[usize::from(x)].protected() {
                x += 1;
            }
            let Some(page) = self.pages.page_mut(node) else {
                return;
            };
            page.release_cells(y, start..x);
            page.get_cells_mut(y)[usize::from(start)..usize::from(x)].fill(blank);
            if start == 0 && x == page.size.cols {
                page.get_row_mut(y).set_kitty_virtual_placeholder(false);
            }
        }
        if let Some(page) = self.pages.page_mut(node) {
            page.refresh_row_metadata(y);
        }
    }

    /// Reset pending wrap; on wrapped rows, also clear continuation and spacer state.
    pub fn cursor_reset_wrap(&mut self) {
        self.cursor.pending_wrap = false;
        let Ok(pin) = self.cursor_pin() else {
            return;
        };
        let Some((row, _)) = pin.row_and_cell_mut(&mut self.pages) else {
            return;
        };
        if !row.wrap() {
            return;
        }
        row.set_wrap(false);
        if let Some(next) = pin.down(&self.pages, 1)
            && let Some((row, _)) = next.row_and_cell_mut(&mut self.pages)
        {
            row.set_wrap_continuation(false);
        }
        self.clear_spacer_head(pin);
    }

    /// Clear wide characters disconnected at a boundary on the cursor row.
    ///
    /// Row wrapping metadata is unchanged.
    ///
    /// # Panics
    /// Panics if the boundary exceeds the stored row width.
    pub fn split_cell_boundary(&mut self, x: u16) {
        let Ok(pin) = self.cursor_pin() else {
            return;
        };
        let Some(page) = self.pages.page(pin.node) else {
            return;
        };
        let cols = page.size.cols;
        assert!(x <= cols);
        let row = page.get_row(pin.y);
        let cells = page.get_cells(pin.y);
        if x == cols {
            if row.wrap() {
                self.clear_spacer_head(pin);
            }
            return;
        }
        let clear_previous = x <= 1 && row.wrap_continuation() && cells[0].wide() == Wide::Wide;
        let clear_pair = x > 0 && cells[usize::from(x - 1)].wide() == Wide::Wide;
        if clear_previous && let Some(previous) = pin.up(&self.pages, 1) {
            self.clear_spacer_head(previous);
        }
        if clear_pair {
            self.clear_cells(pin.node, pin.y, x - 1..x + 1);
        }
    }

    /// Clear a trailing spacer using its own page's stored width.
    fn clear_spacer_head(&mut self, pin: crate::page_list::Pin) {
        let Some(page) = self.pages.page(pin.node) else {
            return;
        };
        let last = page.size.cols - 1;
        if page.get_cells(pin.y)[usize::from(last)].wide() == Wide::SpacerHead {
            self.clear_cells(pin.node, pin.y, last..last + 1);
        }
    }
}
