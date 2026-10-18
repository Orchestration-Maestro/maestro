//! Page capacity and logical byte accounting, independent of native allocations.

use crate::{bitmap_allocator::BitmapAllocator, hash_map, ref_counted_set};

/// Number of addressable columns and rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Columns per row.
    pub cols: u16,
    /// Addressable rows.
    pub rows: u16,
}

/// Minimum logical storage budgets for a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity {
    /// Columns per row.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
    /// Requested style table slots.
    pub styles: u16,
    /// Requested hyperlink entry bytes, excluding string storage.
    pub hyperlink_bytes: u16,
    /// Requested cluster suffix bytes.
    pub grapheme_bytes: u32,
    /// Requested hyperlink string bytes.
    pub string_bytes: u32,
}

/// Default cluster suffix byte budget.
pub const GRAPHEME_BYTES_DEFAULT: u32 = 1024;
/// Default hyperlink string byte budget.
pub const STRING_BYTES_DEFAULT: u32 = 2048;
/// Standard page capacity used by terminal storage.
pub const STD_CAPACITY: Capacity = Capacity {
    cols: 215,
    rows: 215,
    styles: 128,
    hyperlink_bytes: 192,
    grapheme_bytes: 8192,
    string_bytes: STRING_BYTES_DEFAULT,
};

/// Logical region accounting; native collection allocation sizes are not measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Total logical bytes rounded to the target's minimum page size.
    pub total_size: u64,
    /// Requested budgets used to compute these regions.
    pub capacity: Capacity,
    /// Rows byte offset.
    pub(crate) rows_start: u64,
    /// Rows byte size.
    pub(crate) rows_size: u64,
    /// Cells byte offset.
    pub(crate) cells_start: u64,
    /// Cells byte size.
    pub(crate) cells_size: u64,
    /// Style set byte offset.
    pub(crate) styles_start: u64,
    /// Style set's logical regions.
    pub(crate) styles: ref_counted_set::Layout,
    /// Cluster allocator byte offset.
    pub(crate) grapheme_alloc_start: u64,
    /// Cluster allocator's logical regions.
    pub(crate) grapheme_alloc: crate::bitmap_allocator::Layout,
    /// Cluster map byte offset.
    pub(crate) grapheme_map_start: u64,
    /// Cluster map's logical regions.
    pub(crate) grapheme_map: hash_map::Layout,
    /// String allocator byte offset.
    pub(crate) string_alloc_start: u64,
    /// String allocator's logical regions.
    pub(crate) string_alloc: crate::bitmap_allocator::Layout,
    /// Hyperlink map byte offset.
    pub(crate) hyperlink_map_start: u64,
    /// Hyperlink map's logical regions.
    pub(crate) hyperlink_map: hash_map::Layout,
    /// Hyperlink set byte offset.
    pub(crate) hyperlink_set_start: u64,
    /// Hyperlink set's logical regions.
    pub(crate) hyperlink_set: ref_counted_set::Layout,
}

/// Compute every logical region for a supplied host page size.
pub(crate) fn layout_for_page_size(capacity: Capacity, page_size: u64) -> Layout {
    let rows_size = u64::from(capacity.rows) * 8;
    let cells_size = u64::from(capacity.rows) * u64::from(capacity.cols) * 8;
    let styles = ref_counted_set::layout(u64::from(capacity.styles), 36, 4);
    let styles_start = rows_size + cells_size;
    let grapheme_alloc = BitmapAllocator::<16>::layout(u64::from(capacity.grapheme_bytes));
    let grapheme_alloc_start = (styles_start + styles.total_size).next_multiple_of(8);
    let count = u64::from(capacity.grapheme_bytes).div_ceil(16);
    let grapheme_map = hash_map::layout(count, 4, 16, 8, 100);
    let grapheme_map_start = (grapheme_alloc_start + grapheme_alloc.total_size).next_multiple_of(8);
    let string_alloc = BitmapAllocator::<32>::layout(u64::from(capacity.string_bytes));
    let string_alloc_start = (grapheme_map_start + grapheme_map.total_size).next_multiple_of(8);
    let hyperlink_count = u64::from(capacity.hyperlink_bytes) / 48;
    let hyperlink_set = ref_counted_set::layout(hyperlink_count, 48, 8);
    let hyperlink_set_start = (string_alloc_start + string_alloc.total_size).next_multiple_of(8);
    let hyperlink_map = hash_map::layout(hyperlink_count * 16, 4, 2, 2, 80);
    let hyperlink_map_start = (hyperlink_set_start + hyperlink_set.total_size).next_multiple_of(4);
    Layout {
        total_size: (hyperlink_map_start + hyperlink_map.total_size).next_multiple_of(page_size),
        capacity,
        rows_start: 0,
        rows_size,
        cells_start: rows_size,
        cells_size,
        styles_start,
        styles,
        grapheme_alloc_start,
        grapheme_alloc,
        grapheme_map_start,
        grapheme_map,
        string_alloc_start,
        string_alloc,
        hyperlink_map_start,
        hyperlink_map,
        hyperlink_set_start,
        hyperlink_set,
    }
}

#[cfg(test)]
mod tests;

/// Changes to column count while retaining the logical page byte budget.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Adjustment {
    /// New column count, or no adjustment.
    pub cols: Option<u16>,
}

/// Minimum page size of the shipped native target.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(crate) const PAGE_SIZE: u64 = 16_384;
/// Minimum page size of the shipped native target.
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub(crate) const PAGE_SIZE: u64 = 4096;

impl Capacity {
    /// Adjust columns, fitting the largest whole number of rows into the budget.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn adjust(self, request: Adjustment) -> Result<Self, super::CapacityError> {
        let Some(cols) = request.cols else {
            return Ok(self);
        };
        let rows = self.available_grid_bytes() / (8 + 8 * u64::from(cols));
        let rows = u16::try_from(rows).map_err(|_| super::CapacityError)?;
        if rows == 0 {
            return Err(super::CapacityError);
        }
        Ok(Self { cols, rows, ..self })
    }

    /// Largest column count that fits one row in this logical byte budget.
    #[must_use]
    pub fn max_cols(self) -> Option<u16> {
        let bytes = self.available_grid_bytes();
        if bytes <= 8 {
            return None;
        }
        Some(u16::try_from((bytes - 8) / 8).unwrap_or(u16::MAX))
    }

    /// Find bytes available for row metadata and cells after backwards alignment.
    fn available_grid_bytes(self) -> u64 {
        let layout = layout_for_page_size(self, PAGE_SIZE);
        let mut end = layout.total_size;
        for (bytes, alignment) in [
            (layout.hyperlink_map.total_size, 4),
            (layout.hyperlink_set.total_size, 8),
            (layout.string_alloc.total_size, 8),
            (layout.grapheme_map.total_size, 8),
            (layout.grapheme_alloc.total_size, 8),
            (layout.styles.total_size, 4),
        ] {
            end = (end - bytes) / alignment * alignment;
        }
        end
    }
}

impl Default for Capacity {
    fn default() -> Self {
        Self {
            cols: 0,
            rows: 0,
            styles: 16,
            hyperlink_bytes: 192,
            grapheme_bytes: GRAPHEME_BYTES_DEFAULT,
            string_bytes: STRING_BYTES_DEFAULT,
        }
    }
}
