//! Cell transfer with managed-capacity recovery through the page owner.

use super::{IncreaseCapacity, PageList, reflow_cursor::ReflowCursor};
use crate::{
    Page,
    page::{
        CellPosition, StorageError,
        cell::{Cell, Content, Wide},
    },
};

/// Whether a source cell was consumed or needs retrying after a spacer.
pub(super) enum Written {
    /// One source cell consumed.
    One,
    /// A one-column destination consumes the wide cell and its tail.
    Pair,
    /// A spacer head was emitted; retry the same source cell after wrapping.
    Repeat,
}

impl ReflowCursor {
    /// Transfer a source cell, reusing page copying for all managed values.
    pub(super) fn write_cell(
        &mut self,
        list: &mut PageList,
        source: &Page,
        position: CellPosition,
    ) -> Result<Written, StorageError> {
        let cell = source.get_cells(position.y)[usize::from(position.x)];
        let cols = list.cols();
        match cell.wide() {
            Wide::SpacerHead => return Ok(Written::One),
            Wide::Wide if cols == 1 => {
                if let Some(page) = list.page_mut(self.pin.node) {
                    page.get_cells_mut(self.pin.y)[0] = Cell::default();
                }
                self.forward(cols);
                return Ok(Written::Pair);
            }
            Wide::SpacerTail if cols == 1 => return Ok(Written::One),
            Wide::Wide if self.pin.x == cols - 1 => {
                let mut spacer = Cell::default();
                spacer.set_wide(Wide::SpacerHead);
                if let Some(page) = list.page_mut(self.pin.node) {
                    page.get_cells_mut(self.pin.y)[usize::from(self.pin.x)] = spacer;
                }
                self.forward(cols);
                return Ok(Written::Repeat);
            }
            _ => {}
        }
        self.copy_with_growth(list, source, position)?;
        if cell.codepoint() == '\u{10eeee}'
            && let Some(page) = list.page_mut(self.pin.node)
        {
            page.get_row_mut(self.pin.y)
                .set_kitty_virtual_placeholder(true);
        }
        self.forward(cols);
        Ok(Written::One)
    }

    /// Retry each managed-table refusal through the single capacity owner.
    fn copy_with_growth(
        &mut self,
        list: &mut PageList,
        source: &Page,
        position: CellPosition,
    ) -> Result<(), StorageError> {
        loop {
            let page = list
                .page_mut(self.pin.node)
                .ok_or(StorageError::OutOfMemory)?;
            let target = CellPosition {
                x: self.pin.x,
                y: self.pin.y,
            };
            page.clear_cells(target.y, target.x..target.x + 1);
            let Err(error) = page.clone_cell_from(source, target, position) else {
                return Ok(());
            };
            let adjustment = match error {
                StorageError::StyleSetOutOfMemory => IncreaseCapacity::Styles,
                StorageError::GraphemeMapOutOfMemory | StorageError::GraphemeAllocOutOfMemory => {
                    IncreaseCapacity::GraphemeBytes
                }
                StorageError::StringAllocOutOfMemory | StorageError::StringsOutOfMemory => {
                    IncreaseCapacity::StringBytes
                }
                StorageError::HyperlinkSetOutOfMemory
                | StorageError::HyperlinkMapOutOfMemory
                | StorageError::SetOutOfMemory => IncreaseCapacity::HyperlinkBytes,
                StorageError::OutOfMemory => return Err(error),
            };
            if let Ok(node) = list.increase_capacity(self.pin.node, Some(adjustment)) {
                self.pin.node = node;
                continue;
            }
            if self.pin.y > 0 {
                self.move_last_row(list, PageList::resized_capacity(source, list.cols()))?;
                continue;
            }
            self.discard_managed(list, source.get_cells(position.y)[usize::from(position.x)]);
            return Ok(());
        }
    }

    /// A single unsplittable row retains valid unmanaged content at exhaustion.
    fn discard_managed(&self, list: &mut PageList, mut cell: Cell) {
        if let Some(page) = list.page_mut(self.pin.node) {
            page.clear_cells(self.pin.y, self.pin.x..self.pin.x + 1);
            cell.set_content(Content::Codepoint(cell.codepoint()));
            cell.set_style_id(0);
            cell.set_hyperlink(false);
            page.get_cells_mut(self.pin.y)[usize::from(self.pin.x)] = cell;
        }
    }
}
