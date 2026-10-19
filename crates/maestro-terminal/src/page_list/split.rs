//! Split linked pages without changing surviving node lifetimes.

use super::{Node, PageList, Pin};
use crate::Page;

/// A page split could not be performed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitError {
    /// A single row cannot be split further, or managed copying cannot fit.
    OutOfSpace,
    /// The supplied location is stale, foreign or outside current bounds.
    InvalidPin,
}

impl PageList {
    /// Move the selected row and following rows into a new neighbouring page.
    ///
    /// # Errors
    /// Returns a checked position error or a storage refusal without changing rows.
    pub fn split(&mut self, pin: Pin) -> Result<(), SplitError> {
        if !self.pin_is_valid(pin) {
            return Err(SplitError::InvalidPin);
        }
        let source = self.nodes.get(pin.node).ok_or(SplitError::InvalidPin)?;
        if source.page.size.rows <= 1 {
            return Err(SplitError::OutOfSpace);
        }
        if pin.y == 0 {
            return Ok(());
        }
        let old_end = source.page.size.rows;
        let capacity = source.page.capacity;
        let mut page = Page::init(capacity);
        page.size = crate::page::layout::Size {
            cols: source.page.size.cols,
            rows: old_end - pin.y,
        };
        page.clone_from(&source.page, pin.y..old_end)
            .map_err(|_| SplitError::OutOfSpace)?;
        let following = source.next;
        let target = self.nodes.insert(Node {
            page,
            prev: Some(pin.node),
            next: following,
        });
        if let Some(source) = self.nodes.get_mut(pin.node) {
            for y in pin.y..old_end {
                source.page.clear_cells(y, 0..source.page.size.cols);
            }
            source.page.size.rows = pin.y;
            source.next = Some(target);
        }
        if let Some(following) = following.and_then(|id| self.nodes.get_mut(id)) {
            following.prev = Some(target);
        } else {
            self.last = target;
        }
        for tracked in self
            .tracked
            .values_mut()
            .filter(|p| p.node == pin.node && p.y >= pin.y)
        {
            tracked.node = target;
            tracked.y -= pin.y;
        }
        self.page_size += Self::page_charge(capacity);
        Ok(())
    }
}
