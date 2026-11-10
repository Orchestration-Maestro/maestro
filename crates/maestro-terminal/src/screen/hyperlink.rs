//! Cursor-held hyperlink identities and cell ownership.

use super::Screen;
use crate::{
    links::{Hyperlink, HyperlinkId},
    page::{CellPosition, StorageError},
    page_list::{IncreaseCapacity, IncreaseCapacityError as Error},
};

impl Screen {
    /// Begin an owned explicit or wrapping implicit hyperlink.
    ///
    /// # Errors
    /// Returns a capacity error when page storage cannot grow enough for the link.
    pub fn start_hyperlink(&mut self, uri: &[u8], id: Option<&[u8]>) -> Result<(), Error> {
        let implicit = self.cursor.hyperlink_implicit_id;
        let link = Hyperlink {
            uri: uri.to_vec(),
            id: id.map_or(HyperlinkId::Implicit(implicit), |id| {
                HyperlinkId::Explicit(id.to_vec())
            }),
        };
        if id.is_none() {
            self.cursor.hyperlink_implicit_id = implicit.wrapping_add(1);
        }
        let result = self.admit_hyperlink(&link);
        if result.is_err() && id.is_none() {
            self.cursor.hyperlink_implicit_id = implicit;
        }
        result
    }

    /// Retry admission only after the exhausted managed dimension grows.
    pub(super) fn admit_hyperlink(&mut self, link: &Hyperlink) -> Result<(), Error> {
        loop {
            let adjustment = match self.start_hyperlink_once(link.clone()) {
                Ok(()) => return Ok(()),
                Err(StorageError::StringsOutOfMemory) => IncreaseCapacity::StringBytes,
                Err(StorageError::SetOutOfMemory) => IncreaseCapacity::HyperlinkBytes,
                Err(_) => return Err(Error::OutOfSpace),
            };
            self.increase_capacity(self.cursor_pin()?.node, Some(adjustment))?;
        }
    }

    /// Install independently owned bytes before replacing the previous link.
    pub(super) fn start_hyperlink_once(&mut self, link: Hyperlink) -> Result<(), StorageError> {
        self.end_hyperlink();
        let pin = self.cursor_pin().map_err(|_| StorageError::OutOfMemory)?;
        let page = self
            .pages
            .page_mut(pin.node)
            .ok_or(StorageError::OutOfMemory)?;
        let id = page.insert_hyperlink(link.clone())?;
        self.cursor.hyperlink = Some(Box::new(link));
        self.cursor.hyperlink_id = id;
        Ok(())
    }

    /// Release the cursor reference without affecting cells retaining the link.
    pub fn end_hyperlink(&mut self) {
        if self.cursor.hyperlink_id != 0
            && let Ok(pin) = self.cursor_pin()
            && let Some(page) = self.pages.page_mut(pin.node)
        {
            page.release_hyperlink(self.cursor.hyperlink_id);
        }
        self.cursor.hyperlink_id = 0;
        self.cursor.hyperlink = None;
    }

    /// Attach the current hyperlink to the cursor cell, retaining a cell owner.
    ///
    /// # Errors
    /// Returns a capacity error when the linked-cell map cannot grow further.
    ///
    /// # Panics
    /// Panics if no hyperlink is active.
    pub fn cursor_set_hyperlink(&mut self) -> Result<(), Error> {
        assert_ne!(self.cursor.hyperlink_id, 0);
        loop {
            let pin = self.cursor_pin()?;
            let page = self.pages.page_mut(pin.node).ok_or(Error::InvalidNode)?;
            let id = self.cursor.hyperlink_id;
            // Retain before replacement so assigning the same link cannot free its bytes.
            page.hyperlink_set.use_multiple(id, 1);
            match page.set_hyperlink(CellPosition { x: pin.x, y: pin.y }, id) {
                Ok(()) => return Ok(()),
                Err(_) => page.release_hyperlink(id),
            }
            self.increase_capacity(pin.node, Some(IncreaseCapacity::HyperlinkBytes))?;
            if self.cursor.hyperlink_id == 0 {
                return Ok(());
            }
        }
    }
}
