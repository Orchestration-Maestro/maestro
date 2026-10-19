//! Cursor ownership across page replacement and splitting.

use super::{Screen, cursor::LinkRestore};
use crate::{
    Page,
    page_list::{IncreaseCapacity, IncreaseCapacityError as Error, NodeId, Pin, SplitError},
};

impl Screen {
    /// Resolve the screen-owned retained cursor handle.
    pub(super) fn cursor_pin(&self) -> Result<Pin, Error> {
        self.pages
            .tracked_pin(self.cursor.page_pin)
            .copied()
            .ok_or(Error::InvalidNode)
    }

    /// Grow a page's managed storage, restoring cursor-owned references if needed.
    ///
    /// # Errors
    /// Returns the list's typed capacity or stale-node error.
    pub fn increase_capacity(
        &mut self,
        node: NodeId,
        adjustment: Option<IncreaseCapacity>,
    ) -> Result<NodeId, Error> {
        if self.cursor_pin()?.node != node {
            return self.pages.increase_capacity(node, adjustment);
        }
        self.with_cursor_detached(LinkRestore::Preserve, |screen| {
            let new = screen.pages.increase_capacity(node, adjustment)?;
            screen.cursor_reload();
            Ok(new)
        })
    }

    /// Split so the pinned row occupies the less heavily used side.
    pub(super) fn split_for_capacity(&mut self, pin: Pin) -> Result<(), Error> {
        let page = self.pages.page(pin.node).ok_or(Error::InvalidNode)?;
        let above = Page::layout(page.exact_row_capacity(0..pin.y + 1)).total_size;
        let below = Page::layout(page.exact_row_capacity(pin.y..page.size.rows)).total_size;
        let split = if above < below {
            pin.down(&self.pages, 1).unwrap_or(pin)
        } else {
            pin
        };
        self.with_cursor_detached(LinkRestore::PageChange, |screen| {
            let old = screen.cursor_pin()?;
            screen.pages.split(split).map_err(|error| match error {
                SplitError::OutOfSpace => Error::OutOfSpace,
                SplitError::InvalidPin => Error::InvalidNode,
            })?;
            let new = screen.cursor_pin()?;
            if old.node != new.node
                && let Some(pin) = screen.pages.tracked_pin_mut(screen.cursor.page_pin)
            {
                *pin = old;
                screen.cursor_change_pin(new);
            }
            Ok(())
        })
    }
}
