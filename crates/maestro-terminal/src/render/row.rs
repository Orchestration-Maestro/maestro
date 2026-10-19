//! Borrowed viewport rows with independently acknowledged invalidation.

use super::{RenderCell, cell::CellData};
use crate::{color::Palette, page::row::Row};

/// Owned contents of one retained viewport row.
#[derive(Default)]
pub(super) struct RowData {
    /// Copied row metadata.
    pub raw: Row,
    /// Owned cell payloads.
    pub cells: Vec<CellData>,
    /// Unacknowledged renderer invalidation.
    pub dirty: bool,
}

/// A row borrowing the render state, not the terminal's pages.
pub struct RenderRow<'a> {
    /// Retained contents and acknowledgement flag.
    pub(super) data: &'a mut RowData,
    /// Snapshot palette for cell colour resolution.
    pub(super) palette: &'a Palette,
}

impl RenderRow<'_> {
    /// Copied row flags.
    #[must_use]
    pub fn raw(&self) -> Row {
        self.data.raw
    }

    /// Whether this row needs rendering.
    #[must_use]
    pub fn dirty(&self) -> bool {
        self.data.dirty
    }

    /// Acknowledge only this row, leaving all other dirty flags unchanged.
    pub fn clear_dirty(&mut self) {
        self.data.dirty = false;
    }

    /// Borrow all cells in viewport column order.
    #[must_use]
    pub fn cells(&self) -> impl ExactSizeIterator<Item = RenderCell<'_>> {
        self.data.cells.iter().map(|data| RenderCell {
            data,
            palette: self.palette,
        })
    }

    /// Borrow one cell, returning none for an out-of-range column.
    #[must_use]
    pub fn cell(&self, x: u16) -> Option<RenderCell<'_>> {
        Some(RenderCell {
            data: self.data.cells.get(usize::from(x))?,
            palette: self.palette,
        })
    }
}

impl RowData {
    /// Rebuild one dirty row, consuming its source flag before copying metadata.
    pub fn update(&mut self, page: &mut crate::page::Page, y: u16, force: bool) -> bool {
        if !force && !page.get_row(y).dirty() {
            return false;
        }
        page.get_row_mut(y).set_dirty(false);
        self.raw = *page.get_row(y);
        self.cells.clear();
        for (x, raw) in (0..page.size.cols).zip(page.get_cells(y).iter().copied()) {
            self.cells
                .push(CellData::new(page, crate::page::CellPosition { x, y }, raw));
        }
        self.dirty = true;
        true
    }
}
