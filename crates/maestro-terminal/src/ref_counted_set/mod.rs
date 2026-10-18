//! Deduplicated values with explicit page-owned references.

use crate::page::StorageError;
use std::{collections::HashMap, hash::Hash, sync::Arc};

/// A value index with opaque nonzero IDs and reusable released slots.
#[derive(Clone)]
pub(crate) struct RefCountedSet<T> {
    /// Values addressed by the IDs stored in cells.
    values: HashMap<u16, Item<T>>,
    /// One identity index for deduplication.
    ids: HashMap<Arc<T>, u16>,
    /// Released IDs available for another value.
    free: Vec<u16>,
    /// Maximum living values, excluding reserved zero.
    capacity: usize,
    /// Next never-used ID.
    next_id: u16,
}

/// A shared value with its page-local reference count.
#[derive(Clone)]
struct Item<T> {
    /// Ownership retained independently of callers.
    value: Arc<T>,
    /// Number of page-owned references.
    refs: u16,
}

impl<T: Eq + Hash> RefCountedSet<T> {
    /// Reserve logical capacity using the set's 13/16 load factor.
    pub(crate) fn init(capacity: usize) -> Self {
        Self {
            values: HashMap::new(),
            ids: HashMap::new(),
            free: Vec::new(),
            capacity: usize::try_from(layout(capacity as u64, 0, 1).cap)
                .unwrap_or(usize::MAX)
                .saturating_sub(1),
            next_id: 1,
        }
    }

    /// Account for reserved zero and the 13/16 load factor.
    pub(crate) fn capacity_for_count(count: usize) -> usize {
        if count == 0 {
            0
        } else {
            ((count + 1) * 16).div_ceil(13)
        }
    }

    /// Add one owner, deduplicating by the complete value.
    pub(crate) fn add(&mut self, value: T) -> Result<u16, StorageError> {
        if let Some(&id) = self.ids.get(&value) {
            self.use_multiple(id, 1);
            return Ok(id);
        }
        if self.values.len() >= self.capacity {
            return Err(StorageError::OutOfMemory);
        }
        let id = if let Some(id) = self.free.pop() {
            id
        } else {
            let id = self.next_id;
            self.next_id += 1;
            id
        };
        let value = Arc::new(value);
        self.ids.insert(Arc::clone(&value), id);
        self.values.insert(id, Item { value, refs: 1 });
        Ok(id)
    }

    /// Retain additional owners of a resident ID.
    pub(crate) fn use_multiple(&mut self, id: u16, count: u16) {
        if let Some(item) = self.values.get_mut(&id) {
            item.refs += count;
        }
    }

    /// Read a resident value; the caller must retain its ID.
    pub(crate) fn get(&self, id: u16) -> &T {
        &self.values[&id].value
    }

    /// Release owners, returning the value when the final owner is removed.
    pub(crate) fn release_multiple(&mut self, id: u16, count: u16) -> Option<Arc<T>> {
        let item = self.values.get_mut(&id)?;
        item.refs -= count;
        if item.refs != 0 {
            return None;
        }
        let item = self.values.remove(&id)?;
        self.ids.remove(&item.value);
        self.free.push(id);
        Some(item.value)
    }

    /// Return the page-local reference count of a resident ID.
    #[cfg(test)]
    pub(crate) fn ref_count(&self, id: u16) -> u16 {
        self.values[&id].refs
    }

    /// Count living distinct values.
    #[cfg(test)]
    pub(crate) fn count(&self) -> usize {
        self.values.len()
    }

    /// Find an ID by complete value identity.
    pub(crate) fn lookup(&self, value: &T) -> Option<u16> {
        self.ids.get(value).copied()
    }
}

/// Logical table and item regions, independent of native collection storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    /// Item slots including reserved zero.
    pub(crate) cap: u64,
    /// Power-of-two hash table slots.
    pub(crate) table_cap: u64,
    /// Wrapped table index mask.
    pub(crate) table_mask: u16,
    /// Table byte offset.
    pub(crate) table_start: u64,
    /// Aligned item byte offset.
    pub(crate) items_start: u64,
    /// Combined logical byte size.
    pub(crate) total_size: u64,
}

/// Compute the source's byte accounting, not native collection allocation size.
pub(crate) fn layout(capacity: u64, item_bytes: u64, item_align: u64) -> Layout {
    let table_cap = if capacity == 0 {
        0
    } else {
        capacity.next_power_of_two()
    };
    let cap = table_cap * 13 / 16;
    let items_start = (table_cap * 2).div_ceil(item_align) * item_align;
    Layout {
        cap,
        table_cap,
        table_mask: u16::try_from(table_cap.saturating_sub(1)).unwrap_or(u16::MAX),
        table_start: 0,
        items_start,
        total_size: items_start + cap * item_bytes,
    }
}

#[cfg(test)]
mod tests;
