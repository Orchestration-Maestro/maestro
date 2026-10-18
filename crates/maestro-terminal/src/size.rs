//! Numeric bounds of terminal page storage.

/// Count of cells in one row or column.
pub type CellCountInt = u16;
/// Requested style table slots.
pub type StyleCountInt = CellCountInt;
/// Hyperlink entry storage byte count.
pub type HyperlinkCountInt = CellCountInt;
/// Cluster suffix storage byte count.
pub type GraphemeBytesInt = u32;
/// Hyperlink string storage byte count.
pub type StringBytesInt = u32;
/// Maximum page size represented by terminal offsets.
pub const MAX_PAGE_SIZE: u32 = u32::MAX;
