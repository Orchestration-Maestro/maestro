//! Hyperlink identity admission, cell maps and last-owner string release.

use super::{CellPosition, Page, StorageError};
use crate::hyperlink::{Hyperlink, HyperlinkId, Id, PageEntry};

impl Page {
    /// Insert and retain a hyperlink identity, allocating its strings transactionally.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn insert_hyperlink(&mut self, link: Hyperlink) -> Result<Id, StorageError> {
        if let Some(id) = self.hyperlink_set.lookup(&link) {
            self.hyperlink_set.use_multiple(id, 1);
            return Ok(id);
        }
        let uri_chunks = self
            .string_alloc
            .alloc(link.uri.len())
            .map_err(|_| StorageError::StringsOutOfMemory)?;
        let id_chunks = if let HyperlinkId::Explicit(bytes) = &link.id {
            if let Ok(chunks) = self.string_alloc.alloc(bytes.len()) {
                Some(chunks)
            } else {
                self.string_alloc.free(uri_chunks);
                return Err(StorageError::StringsOutOfMemory);
            }
        } else {
            None
        };
        let chunks = PageEntry {
            uri_chunks,
            id_chunks,
        };
        if let Ok(id) = self.hyperlink_set.add(link) {
            self.hyperlink_entries.insert(id, chunks);
            Ok(id)
        } else {
            self.free_link_strings(chunks);
            Err(StorageError::SetOutOfMemory)
        }
    }

    /// Assign a retained identity, consuming its existing reference without incrementing it.
    /// On map exhaustion the caller still owns that identity reference.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn set_hyperlink(&mut self, position: CellPosition, id: Id) -> Result<(), StorageError> {
        let index = self.cell_index(position);
        if !self.hyperlink_map.contains_key(&index)
            && self.hyperlink_count() >= self.hyperlink_capacity()
        {
            return Err(StorageError::HyperlinkMapOutOfMemory);
        }
        if let Some(previous) = self.hyperlink_map.insert(index, id) {
            self.release_hyperlink(previous);
        }
        self.cells[index].set_hyperlink(true);
        self.get_row_mut(position.y).set_hyperlink(true);
        Ok(())
    }

    /// Look up a linked cell's page-local identity.
    #[must_use]
    pub fn lookup_hyperlink(&self, position: CellPosition) -> Option<Id> {
        self.hyperlink_map.get(&self.cell_index(position)).copied()
    }

    /// Remove a cell's link reference; refresh row metadata separately after the batch.
    pub fn clear_hyperlink(&mut self, position: CellPosition) {
        let index = self.cell_index(position);
        if let Some(id) = self.hyperlink_map.remove(&index) {
            self.release_hyperlink(id);
            self.cells[index].set_hyperlink(false);
        }
    }

    /// Clear the row flag when no addressable cell retains a hyperlink.
    pub fn update_row_hyperlink_flag(&mut self, y: u16) {
        if !self.get_cells(y).iter().any(|cell| cell.hyperlink()) {
            self.get_row_mut(y).set_hyperlink(false);
        }
    }

    /// Number of linked cells, not the number of distinct identities.
    #[must_use]
    pub fn hyperlink_count(&self) -> usize {
        self.hyperlink_map.len()
    }

    /// Maximum admitted linked cells in the logical map layout.
    #[must_use]
    pub fn hyperlink_capacity(&self) -> usize {
        usize::try_from(Self::layout(self.capacity).hyperlink_map.capacity * 80 / 100)
            .unwrap_or(usize::MAX)
    }

    /// Release a caller-retained identity, reclaiming strings after its last owner.
    /// Do not release a reference already consumed by a successful cell assignment.
    pub fn release_hyperlink(&mut self, id: Id) {
        if self.hyperlink_set.release_multiple(id, 1).is_some()
            && let Some(entry) = self.hyperlink_entries.remove(&id)
        {
            self.free_link_strings(entry);
        }
    }

    /// Return both string allocations associated with a link identity.
    fn free_link_strings(&mut self, entry: PageEntry) {
        self.string_alloc.free(entry.uri_chunks);
        if let Some(chunks) = entry.id_chunks {
            self.string_alloc.free(chunks);
        }
    }
}
