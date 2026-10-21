//! Page-owned cluster suffix values and first-fit chunk accounting.

use super::{CellPosition, Page, StorageError, cell::Content};
use std::ops::Range;

/// A suffix value with its exclusively owned logical chunks.
#[derive(Clone)]
pub(super) struct Grapheme {
    /// Scalars after the cell's first scalar.
    cps: Vec<char>,
    /// Allocated 16-byte chunks.
    chunks: Range<usize>,
}

impl Page {
    /// Append a scalar to the suffix of a non-NUL textual cell.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn append_grapheme(
        &mut self,
        position: CellPosition,
        cp: char,
    ) -> Result<(), StorageError> {
        let index = self.cell_index(position);
        if let Some(entry) = self.graphemes.get_mut(&index) {
            if entry.cps.len().is_multiple_of(4) {
                let bytes = crate::bitmap_allocator::BitmapAllocator::<16>::bytes_required::<char>(
                    entry.cps.len() + 1,
                );
                let chunks = self.grapheme_alloc.alloc(bytes)?;
                let mut cps = Vec::with_capacity(bytes / 4);
                cps.extend_from_slice(&entry.cps);
                entry.cps = cps;
                let old = std::mem::replace(&mut entry.chunks, chunks);
                self.grapheme_alloc.free(old);
            }
            entry.cps.push(cp);
        } else {
            self.set_graphemes(position, &[cp])
                .map_err(|_| StorageError::OutOfMemory)?;
        }
        Ok(())
    }

    /// Set a previously absent suffix, releasing the allocation on map exhaustion.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn set_graphemes(
        &mut self,
        position: CellPosition,
        cps: &[char],
    ) -> Result<(), StorageError> {
        let index = self.cell_index(position);
        let bytes =
            crate::bitmap_allocator::BitmapAllocator::<16>::bytes_required::<char>(cps.len());
        let chunks = self
            .grapheme_alloc
            .alloc(bytes)
            .map_err(|_| StorageError::GraphemeAllocOutOfMemory)?;
        if self.graphemes.len() >= self.grapheme_capacity() {
            self.grapheme_alloc.free(chunks);
            return Err(StorageError::GraphemeMapOutOfMemory);
        }
        let mut values = Vec::with_capacity(bytes / 4);
        values.extend_from_slice(cps);
        self.graphemes.insert(
            index,
            Grapheme {
                cps: values,
                chunks,
            },
        );
        let first = self.cells[index].codepoint();
        self.cells[index].set_content(Content::CodepointGrapheme(first));
        self.rows[usize::from(position.y)].set_grapheme(true);
        Ok(())
    }

    /// Borrow suffix scalars, excluding the cell's first scalar.
    #[must_use]
    pub fn lookup_grapheme(&self, position: CellPosition) -> Option<&[char]> {
        self.graphemes
            .get(&self.cell_index(position))
            .map(|entry| entry.cps.as_slice())
    }

    /// Release a suffix; refresh row metadata separately after the batch.
    pub fn clear_grapheme(&mut self, position: CellPosition) {
        self.take_grapheme(position);
    }

    /// Transfer suffix ownership out of a cell before its page can move or be pruned.
    pub(crate) fn take_grapheme(&mut self, position: CellPosition) -> Option<Vec<char>> {
        let index = self.cell_index(position);
        let entry = self.graphemes.remove(&index)?;
        self.grapheme_alloc.free(entry.chunks);
        let first = self.cells[index].codepoint();
        self.cells[index].set_content(Content::Codepoint(first));
        Some(entry.cps)
    }

    /// Clear the row's suffix flag when no addressable cell retains one.
    pub fn update_row_grapheme_flag(&mut self, y: u16) {
        #[cfg(test)]
        let mut visits = 0;
        let has_grapheme = self.get_cells(y).iter().any(|cell| {
            #[cfg(test)]
            {
                visits += 1;
            }
            cell.has_grapheme()
        });
        #[cfg(test)]
        {
            crate::tests::WORK.with_borrow_mut(|work| work.row_shift += visits);
        }
        if !has_grapheme {
            self.rows[usize::from(y)].set_grapheme(false);
        }
    }

    /// Number of cells currently owning suffixes.
    #[must_use]
    pub fn grapheme_count(&self) -> usize {
        self.graphemes.len()
    }

    /// Maximum suffix-map entries in this page's logical layout.
    #[must_use]
    pub fn grapheme_capacity(&self) -> usize {
        usize::try_from(Self::layout(self.capacity).grapheme_map.capacity).unwrap_or(usize::MAX)
    }
}

impl Page {
    /// Move a suffix map entry without changing either cell's packed fields.
    pub fn move_grapheme(&mut self, src: CellPosition, dst: CellPosition) {
        if let Some(entry) = self.graphemes.remove(&self.cell_index(src)) {
            self.graphemes.insert(self.cell_index(dst), entry);
        }
    }
}
