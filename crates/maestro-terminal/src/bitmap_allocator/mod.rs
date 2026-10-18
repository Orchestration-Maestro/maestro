//! First-fit allocation of fixed-size chunks.

use crate::page::StorageError;
use std::ops::Range;

/// First-fit ownership of power-of-two byte chunks; values live with callers.
#[derive(Clone)]
pub(crate) struct BitmapAllocator<const CHUNK: usize> {
    /// One free bit per addressable chunk.
    bitmap: Vec<u64>,
}

/// Logical byte regions of a chunk allocator, independent of native vectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    /// Combined bitmap and addressable chunk bytes.
    pub(crate) total_size: u64,
    /// Number of 64-bit bitmap words.
    pub(crate) bitmap_count: u64,
    /// Offset of the bitmap region.
    pub(crate) bitmap_start: u64,
    /// Offset of the chunk region.
    pub(crate) chunks_start: u64,
}

impl<const CHUNK: usize> BitmapAllocator<CHUNK> {
    /// Mark every chunk in the rounded capacity free.
    pub(crate) fn init(bytes: u32) -> Self {
        Self {
            bitmap: vec![u64::MAX; (bytes as usize).div_ceil(CHUNK * 64)],
        }
    }

    /// Reserve a contiguous range of chunk indices for a positive byte count.
    pub(crate) fn alloc(&mut self, bytes: usize) -> Result<Range<usize>, StorageError> {
        let count = bytes.div_ceil(CHUNK);
        let start = find_free_chunks(&mut self.bitmap, count).ok_or(StorageError::OutOfMemory)?;
        Ok(start..start + count)
    }

    /// Release a previously allocated chunk range.
    pub(crate) fn free(&mut self, chunks: Range<usize>) {
        mark_chunks(&mut self.bitmap, chunks, true);
    }

    /// Return the byte capacity addressable by the bitmap.
    #[cfg(test)]
    pub(crate) fn capacity_bytes(&self) -> usize {
        self.bitmap.len() * 64 * CHUNK
    }

    /// Count occupied chunks in bytes.
    #[cfg(test)]
    pub(crate) fn used_bytes(&self) -> usize {
        self.bitmap
            .iter()
            .map(|word| word.count_zeros() as usize * CHUNK)
            .sum()
    }

    /// Compute bitmap bytes and every chunk addressable by those bits.
    pub(crate) fn layout(bytes: u64) -> Layout {
        let bitmap_count = bytes.div_ceil(CHUNK as u64).div_ceil(64);
        let chunks_start = bitmap_count * 8;
        Layout {
            total_size: chunks_start + bitmap_count * 64 * CHUNK as u64,
            bitmap_count,
            bitmap_start: 0,
            chunks_start,
        }
    }

    /// Round a typed element count to whole chunks.
    pub(crate) fn bytes_required<T>(count: usize) -> usize {
        (std::mem::size_of::<T>() * count).div_ceil(CHUNK) * CHUNK
    }
}

/// Claim the first free run, using word-local search for short spans.
fn find_free_chunks(bitmaps: &mut [u64], count: usize) -> Option<usize> {
    if count > 64 {
        return find_large_span(bitmaps, count);
    }
    for (index, word) in bitmaps.iter_mut().enumerate() {
        let mut starts = *word;
        for shift in 1..count {
            starts &= *word >> shift;
        }
        if starts != 0 {
            let bit = starts.trailing_zeros();
            let mask = (u64::MAX >> (64 - count)) << bit;
            *word &= !mask;
            return Some(index * 64 + bit as usize);
        }
    }
    None
}

/// Find a run spanning at least two bitmap words.
fn find_large_span(bitmaps: &mut [u64], count: usize) -> Option<usize> {
    let mut start = 0;
    while start < bitmaps.len() {
        let prefix = bitmaps[start].leading_ones() as usize;
        if prefix == 0 {
            start += 1;
            continue;
        }
        let mut remaining = count - prefix;
        let mut end = start + 1;
        while remaining > 64 && bitmaps.get(end) == Some(&u64::MAX) {
            remaining -= 64;
            end += 1;
        }
        let word = *bitmaps.get(end)?;
        if remaining <= 64 && word.trailing_ones() as usize >= remaining {
            let first = start * 64 + 64 - prefix;
            mark_chunks(bitmaps, first..first + count, false);
            return Some(first);
        }
        start = end;
    }
    None
}

/// Update only the bits belonging to a chunk range.
fn mark_chunks(bitmaps: &mut [u64], chunks: std::ops::Range<usize>, free: bool) {
    let mut start = chunks.start;
    while start < chunks.end {
        let bit = start % 64;
        let count = (chunks.end - start).min(64 - bit);
        let mask = (u64::MAX >> (64 - count)) << bit;
        let word = &mut bitmaps[start / 64];
        if free {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        start += count;
    }
}

#[cfg(test)]
mod tests;
