//! Reverse suffix rotation with managed copies only at page boundaries.

use super::{PageList, Pin};
use crate::page::StorageError;

impl PageList {
    /// Move a suffix down into the appended row without visiting earlier history.
    pub(crate) fn rotate_rows_down_from(&mut self, pin: Pin) -> Result<(), StorageError> {
        let mut current = self.last;
        while current != pin.node {
            let Some(previous) = self.nodes.get(current).and_then(|node| node.prev) else {
                return Ok(());
            };
            if let Some((source, destination)) = self.nodes.get_pair_mut(previous, current) {
                let rows = destination.page.size.rows;
                destination.page.rotate_rows(0..rows, usize::from(rows - 1));
                destination
                    .page
                    .clone_row_from(&source.page, 0, source.page.size.rows - 1)?;
                destination.page.dirty = true;
            }
            current = previous;
        }
        if let Some(page) = self.page_mut(current) {
            let end = page.size.rows;
            page.rotate_rows(pin.y..end, usize::from(end - pin.y - 1));
            page.dirty = true;
        }
        Ok(())
    }
}
