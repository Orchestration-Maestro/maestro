//! Managed row transfers between resident pages.

use super::{PageList, Pin};
use crate::page::{CellPosition, StorageError};

impl PageList {
    /// Transfer selected cells, copying managed values only at a page boundary.
    pub(crate) fn transfer_row(
        &mut self,
        src: Pin,
        dst: Pin,
        columns: std::ops::Range<u16>,
    ) -> Result<(), StorageError> {
        if src.node != dst.node {
            let (source, destination) = self
                .nodes
                .get_pair_mut(src.node, dst.node)
                .ok_or(StorageError::OutOfMemory)?;
            return destination
                .page
                .clone_partial_row_from(&source.page, dst.y, src.y, columns);
        }
        let page = self.page_mut(src.node).ok_or(StorageError::OutOfMemory)?;
        if columns.start == 0 && columns.end == page.size.cols {
            for x in columns {
                page.swap_cells(CellPosition { x, y: src.y }, CellPosition { x, y: dst.y });
            }
            let source = *page.get_row(src.y);
            let destination = *page.get_row(dst.y);
            *page.get_row_mut(src.y) = destination;
            *page.get_row_mut(dst.y) = source;
        } else {
            page.move_cells(
                CellPosition {
                    x: columns.start,
                    y: src.y,
                },
                CellPosition {
                    x: columns.start,
                    y: dst.y,
                },
                columns.end - columns.start,
            );
        }
        Ok(())
    }
}
