//! Cursor ownership across page replacement and splitting.

use super::Screen;
use crate::{
    Page,
    page_list::{IncreaseCapacity, IncreaseCapacityError as Error, NodeId, Pin, SplitError},
    style::Style,
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
        let cursor_page = self.cursor_pin()?.node == node;
        let new = self.pages.increase_capacity(node, adjustment)?;
        if !cursor_page {
            return Ok(new);
        }
        if self.cursor.style_id != 0 {
            if let Ok(id) = self
                .pages
                .page_mut(new)
                .ok_or(Error::InvalidNode)?
                .styles
                .add(self.cursor.style)
            {
                self.cursor.style_id = id;
            } else {
                self.cursor.style = Style::default();
                self.cursor.style_id = 0;
            }
        }
        if let Some(link) = self.cursor.hyperlink.take() {
            self.cursor.hyperlink_id = 0;
            let _ = self.start_hyperlink_once(*link);
        }
        self.cursor_reload();
        Ok(new)
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
        let old = self.cursor_pin()?;
        self.pages.split(split).map_err(|error| match error {
            SplitError::OutOfSpace => Error::OutOfSpace,
            SplitError::InvalidPin => Error::InvalidNode,
        })?;
        let new = self.cursor_pin()?;
        if new.node != old.node {
            if let Some(pin) = self.pages.tracked_pin_mut(self.cursor.page_pin) {
                *pin = old;
            }
            self.cursor_change_pin(new);
        }
        Ok(())
    }
}
