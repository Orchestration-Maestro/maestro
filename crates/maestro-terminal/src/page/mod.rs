//! Packed terminal records and page-owned storage.

pub mod cell;
pub mod row;
pub use cell::Cell;

/// A page-owned storage capacity was exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    /// A fixed byte or entry budget cannot admit the requested value.
    OutOfMemory,
    /// The style table cannot admit another value.
    StyleSetOutOfMemory,
    /// A copied link cannot fit the hyperlink set.
    HyperlinkSetOutOfMemory,
    /// Another linked cell cannot fit the hyperlink map.
    HyperlinkMapOutOfMemory,
    /// A copied link cannot fit the string allocator.
    StringAllocOutOfMemory,
    /// Link insertion cannot allocate its strings.
    StringsOutOfMemory,
    /// Link insertion cannot admit its identity.
    SetOutOfMemory,
    /// Another suffix cannot fit the cluster map.
    GraphemeMapOutOfMemory,
    /// A suffix cannot fit the cluster allocator.
    GraphemeAllocOutOfMemory,
}

pub mod layout;

/// A requested capacity adjustment cannot fit a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapacityError;

mod mask;

use self::{
    layout::{Capacity, Layout, Size},
    row::Row,
};
use crate::bitmap_allocator::BitmapAllocator;
use std::collections::HashMap;

mod graphemes;
mod hyperlinks;

/// Owned rows, cells and the storage used by their managed content.
pub struct Page {
    /// Current addressable dimensions, bounded by capacity.
    pub size: Size,
    /// Fixed storage budgets chosen at initialization.
    pub capacity: Capacity,
    /// Whole-page dirty flag, independent of row dirty flags.
    pub dirty: bool,
    /// Packed row metadata in display order.
    pub(crate) rows: Vec<Row>,
    /// Physical cell storage, addressed by each logical row's offset.
    cells: Vec<Cell>,
    /// Physical cell origin owned by each logical row.
    row_offsets: Vec<usize>,
    /// First-fit chunk ownership of cluster suffixes.
    grapheme_alloc: BitmapAllocator<16>,
    /// Suffix values indexed by physical cell position.
    graphemes: HashMap<usize, graphemes::Grapheme>,
    /// Deduplicated styles addressed by packed cell IDs.
    pub(crate) styles: crate::style::Set,
    /// First-fit 32-byte chunks for link strings.
    string_alloc: BitmapAllocator<32>,
    /// Deduplicated hyperlink identity ownership.
    pub(crate) hyperlink_set: crate::links::Set,
    /// Linked cells addressed by physical cell position.
    hyperlink_map: HashMap<usize, crate::links::Id>,
    /// Logical string chunks released with the last identity owner.
    hyperlink_entries: HashMap<crate::links::Id, crate::links::PageEntry>,
}

impl Page {
    /// Initialize zeroed cells and row metadata; at least one row is required.
    #[must_use]
    ///
    /// # Panics
    /// Panics when the requested row count is zero.
    pub fn init(capacity: Capacity) -> Self {
        assert!(capacity.rows > 0);
        Self {
            size: Size {
                cols: capacity.cols,
                rows: capacity.rows,
            },
            capacity,
            dirty: false,
            rows: vec![Row::default(); usize::from(capacity.rows)],
            cells: vec![Cell::default(); usize::from(capacity.cols) * usize::from(capacity.rows)],
            row_offsets: (0..usize::from(capacity.rows))
                .map(|y| y * usize::from(capacity.cols))
                .collect(),
            grapheme_alloc: BitmapAllocator::init(capacity.grapheme_bytes),
            graphemes: HashMap::new(),
            styles: crate::style::Set::init(usize::from(capacity.styles)),
            string_alloc: BitmapAllocator::init(capacity.string_bytes),
            hyperlink_set: crate::links::Set::init(usize::from(capacity.hyperlink_bytes) / 48),
            hyperlink_map: HashMap::new(),
            hyperlink_entries: HashMap::new(),
        }
    }

    /// Resolve packed background content or the page-local text style.
    pub(crate) fn cell_style(&self, cell: Cell) -> crate::Style {
        use crate::style::{Color, Style};
        match cell.content() {
            cell::Content::BgColorPalette(index) => Style {
                bg_color: Color::Palette(index),
                ..Style::default()
            },
            cell::Content::BgColorRgb(rgb) => Style {
                bg_color: Color::Rgb(rgb),
                ..Style::default()
            },
            _ if cell.style_id() != 0 => *self.styles.get(cell.style_id()),
            _ => Style::default(),
        }
    }

    /// Borrow row metadata at the supplied row index.
    #[must_use]
    pub fn get_row(&self, y: u16) -> &Row {
        &self.rows[usize::from(y)]
    }

    /// Borrow the addressable cells of one row.
    #[must_use]
    pub fn get_cells(&self, y: u16) -> &[Cell] {
        let start = self.row_offsets[usize::from(y)];
        &self.cells[start..start + usize::from(self.size.cols)]
    }

    /// Whether the page flag or any addressable row is dirty.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
            || self.rows[..usize::from(self.size.rows)]
                .iter()
                .any(|row| row.dirty())
    }

    /// Compute logical byte accounting on the native target.
    #[must_use]
    pub fn layout(capacity: Capacity) -> Layout {
        layout::layout_for_page_size(capacity, layout::PAGE_SIZE)
    }
}

/// A cell coordinate within a page's addressable dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellPosition {
    /// Column index.
    pub x: u16,
    /// Row index.
    pub y: u16,
}

impl Page {
    /// Mutably borrow one row's metadata.
    pub fn get_row_mut(&mut self, y: u16) -> &mut Row {
        &mut self.rows[usize::from(y)]
    }

    /// Mutably borrow one row's cells; callers maintain managed-table ownership.
    pub fn get_cells_mut(&mut self, y: u16) -> &mut [Cell] {
        let start = self.row_offsets[usize::from(y)];
        &mut self.cells[start..start + usize::from(self.size.cols)]
    }

    /// Mutably borrow a cell together with its row metadata.
    pub fn get_row_and_cell(&mut self, position: CellPosition) -> (&mut Row, &mut Cell) {
        let index = self.cell_index(position);
        (
            &mut self.rows[usize::from(position.y)],
            &mut self.cells[index],
        )
    }

    /// Resolve a coordinate through its row-owned physical cell origin.
    fn cell_index(&self, position: CellPosition) -> usize {
        self.row_offsets[usize::from(position.y)] + usize::from(position.x)
    }

    /// Exchange logical rows without touching their physical cells or managed maps.
    pub(crate) fn swap_rows(&mut self, source: u16, destination: u16) {
        self.rows
            .swap(usize::from(source), usize::from(destination));
        self.row_offsets
            .swap(usize::from(source), usize::from(destination));
    }

    /// Rotate logical rows together with their physical cell ownership.
    pub(crate) fn rotate_rows(&mut self, rows: std::ops::Range<u16>, amount: usize) {
        let rows = usize::from(rows.start)..usize::from(rows.end);
        self.rows[rows.clone()].rotate_left(amount);
        self.row_offsets[rows].rotate_left(amount);
    }
}

#[cfg(test)]
mod tests;

impl Clone for Page {
    fn clone(&self) -> Self {
        Self {
            size: self.size,
            capacity: self.capacity,
            dirty: self.dirty,
            rows: self.rows.clone(),
            cells: self.cells.clone(),
            row_offsets: self.row_offsets.clone(),
            grapheme_alloc: self.grapheme_alloc.clone(),
            graphemes: self.graphemes.clone(),
            styles: self.styles.clone(),
            string_alloc: self.string_alloc.clone(),
            hyperlink_set: self.hyperlink_set.clone(),
            hyperlink_map: self.hyperlink_map.clone(),
            hyperlink_entries: self.hyperlink_entries.clone(),
        }
    }
}

mod cells;
mod clone;

impl Page {
    /// Reset dimensions, rows, cells, owned tables and dirty state at the same capacity.
    pub fn reinit(&mut self) {
        *self = Self::init(self.capacity);
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StorageError {}
impl std::fmt::Display for CapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OutOfMemory")
    }
}
impl std::error::Error for CapacityError {}
