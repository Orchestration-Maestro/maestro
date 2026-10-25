//! Owned destination position for streaming line rewrapping.

use super::{Node, NodeId, PageList, Pin};
use crate::{
    Page,
    page::{StorageError, layout::Capacity},
};

/// Destination position and deferred hard line breaks.
pub(super) struct ReflowCursor {
    /// Current destination cell.
    pub(super) pin: Pin,
    /// The next cell must start a soft-wrapped row.
    pub(super) pending_wrap: bool,
    /// Hard line breaks not yet emitted, including trailing blanks.
    pub(super) new_rows: usize,
    /// Rows actually emitted so far.
    pub(super) total_rows: usize,
}

impl ReflowCursor {
    /// Start at the first cell of a fresh one-row page.
    pub(super) fn new(node: NodeId) -> Self {
        Self {
            pin: Pin {
                node,
                x: 0,
                y: 0,
                garbage: false,
            },
            pending_wrap: false,
            new_rows: 0,
            total_rows: 1,
        }
    }

    /// Advance within a row, retaining the final column until wrapping.
    pub(super) fn forward(&mut self, cols: u16) {
        if self.pin.x == cols - 1 {
            self.pending_wrap = true;
        } else {
            self.pin.x += 1;
        }
    }

    /// Append a row to the current page, or allocate its next neighbour.
    pub(super) fn next_row(&mut self, list: &mut PageList, cap: Capacity) {
        let Some(page) = list.page_mut(self.pin.node) else {
            return;
        };
        if page.size.rows < page.capacity.rows {
            page.size.rows += 1;
            self.pin.y += 1;
        } else {
            let mut page = Page::init(cap);
            page.size.rows = 1;
            self.pin.node = Self::append_page(list, page);
            self.pin.y = 0;
        }
        self.pin.x = 0;
        self.pending_wrap = false;
        self.total_rows += 1;
    }

    /// Link a page at the emitted end without pruning unread source content.
    fn append_page(list: &mut PageList, page: Page) -> NodeId {
        list.page_size += PageList::page_charge(page.capacity);
        let previous = list.last;
        let new = list.nodes.insert(Node {
            page,
            prev: Some(previous),
            next: None,
        });
        if let Some(previous) = list.nodes.get_mut(previous) {
            previous.next = Some(new);
        }
        list.last = new;
        new
    }

    /// Move only the current row when managed storage cannot grow further.
    pub(super) fn move_last_row(
        &mut self,
        list: &mut PageList,
        cap: Capacity,
    ) -> Result<(), StorageError> {
        let source = list.page(self.pin.node).ok_or(StorageError::OutOfMemory)?;
        let mut page = Page::init(cap);
        page.size.rows = 1;
        page.clone_row_from(source, 0, self.pin.y)?;
        let old = self.pin;
        let new = Self::append_page(list, page);
        for pin in list
            .tracked
            .values_mut()
            .filter(|p| p.node == old.node && p.y == old.y)
        {
            pin.node = new;
            pin.y = 0;
        }
        if let Some(page) = list.page_mut(old.node) {
            page.clear_cells(old.y, 0..page.size.cols);
            page.size.rows -= 1;
        }
        if old.y == 0 {
            list.erase_page(old.node);
        }
        self.pin.node = new;
        self.pin.y = 0;
        Ok(())
    }
}
