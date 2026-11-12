//! Cell locators and list-owned retained handles.

use super::{NodeId, PageList};
use crate::page::{Cell as PackedCell, CellPosition, row::Row};

/// Inclusive portion of the row containing a pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellSubset {
    /// Every addressable cell in the row.
    All,
    /// Cells from the row origin through the pin.
    Left,
    /// Cells from the pin through the row end.
    Right,
}

impl CellSubset {
    /// Restrict a row slice to the selected inclusive columns.
    fn range(self, x: u16, len: usize) -> std::ops::Range<usize> {
        match self {
            Self::All => 0..len,
            Self::Left => 0..usize::from(x) + 1,
            Self::Right => usize::from(x)..len,
        }
    }
}

/// A generation-checked handle to a list-owned retained cell position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrackedPin(pub(super) NodeId);

/// An owned cell coordinate in a particular page lifetime.
#[derive(Clone, Copy, Debug)]
pub struct Pin {
    /// Current page lifetime.
    pub node: NodeId,
    /// Zero-based column.
    pub x: u16,
    /// Zero-based page-local row.
    pub y: u16,
    /// Whether an edit removed the formerly tracked content.
    pub garbage: bool,
}

/// Borrowed cell and its row metadata.
pub struct Cell<'a> {
    /// Page lifetime owning these borrows.
    pub node: NodeId,
    /// Row metadata.
    pub row: &'a Row,
    /// Packed cell record.
    pub cell: &'a PackedCell,
    /// Page-local row.
    pub row_idx: u16,
    /// Column within the row.
    pub col_idx: u16,
}

impl Pin {
    /// Borrow row metadata and packed cell after checking current bounds.
    #[must_use]
    pub fn row_and_cell(self, list: &PageList) -> Option<(&Row, &PackedCell)> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let page = list.page(self.node)?;
        Some((
            page.get_row(self.y),
            &page.get_cells(self.y)[usize::from(self.x)],
        ))
    }

    /// Mutably borrow the actual row metadata and packed cell.
    pub fn row_and_cell_mut(self, list: &mut PageList) -> Option<(&mut Row, &mut PackedCell)> {
        if !list.pin_is_valid(self) {
            return None;
        }
        Some(list.page_mut(self.node)?.get_row_and_cell(CellPosition {
            x: self.x,
            y: self.y,
        }))
    }

    /// Borrow an inclusive subset of the actual row cells.
    #[must_use]
    pub fn cells(self, list: &PageList, subset: CellSubset) -> Option<&[PackedCell]> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let cells = list.page(self.node)?.get_cells(self.y);
        Some(&cells[subset.range(self.x, cells.len())])
    }

    /// Mutably borrow an inclusive subset of the actual row cells.
    pub fn cells_mut(self, list: &mut PageList, subset: CellSubset) -> Option<&mut [PackedCell]> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let cells = list.page_mut(self.node)?.get_cells_mut(self.y);
        let range = subset.range(self.x, cells.len());
        Some(&mut cells[range])
    }

    /// Borrow the selected cell's suffix scalars, excluding its first scalar.
    #[must_use]
    pub fn grapheme(self, list: &PageList, cell: CellPosition) -> Option<&[char]> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let page = list.page(self.node)?;
        if cell.x >= page.size.cols || cell.y >= page.size.rows {
            return None;
        }
        page.lookup_grapheme(cell)
    }

    /// Borrow the byte-exact hyperlink URI at this cell, if present.
    #[must_use]
    pub fn hyperlink_uri(self, list: &PageList) -> Option<&[u8]> {
        let (_, cell) = self.row_and_cell(list)?;
        if !cell.hyperlink() {
            return None;
        }
        let page = list.page(self.node)?;
        let id = page.lookup_hyperlink(CellPosition {
            x: self.x,
            y: self.y,
        })?;
        Some(&page.hyperlink_set.get(id).uri)
    }

    /// Resolve the supplied packed cell's style in this pin's page.
    #[must_use]
    pub fn style(self, list: &PageList, cell: PackedCell) -> Option<crate::style::Style> {
        if !list.pin_is_valid(self) {
            return None;
        }
        if cell.style_id() == crate::style::DEFAULT_ID {
            return Some(crate::style::Style::default());
        }
        Some(*list.page(self.node)?.styles.get(cell.style_id()))
    }

    /// Whether this pin's row or its entire page is dirty.
    #[must_use]
    pub fn is_dirty(self, list: &PageList) -> Option<bool> {
        let (row, _) = self.row_and_cell(list)?;
        Some(list.page(self.node)?.dirty || row.dirty())
    }

    /// Mark only this pin's row dirty, leaving the page flag unchanged.
    pub fn mark_dirty(self, list: &mut PageList) {
        if let Some((row, _)) = self.row_and_cell_mut(list) {
            row.set_dirty(true);
        }
    }

    /// Compare page lifetime and coordinates, ignoring garbage state.
    #[must_use]
    pub fn eql(self, other: Self) -> bool {
        self.node == other.node && self.x == other.x && self.y == other.y
    }
}

impl PartialEq for Pin {
    fn eq(&self, other: &Self) -> bool {
        self.eql(*other)
    }
}
impl Eq for Pin {}

impl PageList {
    /// Resolve a region's origin from the retained linked pages.
    #[must_use]
    pub fn get_top_left(&self, tag: crate::point::Tag) -> Pin {
        let origin = Pin {
            node: self.first,
            x: 0,
            y: 0,
            garbage: false,
        };
        match tag {
            crate::point::Tag::Screen | crate::point::Tag::History => return origin,
            crate::point::Tag::Viewport => return self.viewport_origin(),
            crate::point::Tag::Active => {}
        }
        let mut current = Some(self.last);
        let mut remaining = self.rows();
        while let Some(key) = current {
            let Some(node) = self.nodes.get(key) else {
                break;
            };
            if remaining <= node.page.size.rows {
                return Pin {
                    node: key,
                    y: node.page.size.rows - remaining,
                    ..origin
                };
            }
            remaining -= node.page.size.rows;
            current = node.prev;
        }
        origin
    }

    /// Project a current position relative to the requested region's origin.
    #[must_use]
    pub fn point_from_pin(&self, tag: crate::point::Tag, pin: Pin) -> Option<crate::Point> {
        if !self.pin_is_valid(pin) {
            return None;
        }
        let origin = self.get_top_left(tag);
        let mut current = Some(origin.node);
        let mut rows = 0_u32;
        while let Some(key) = current {
            let page = self.page(key)?;
            let start = if key == origin.node { origin.y } else { 0 };
            if key == pin.node {
                let offset = pin.y.checked_sub(start)?;
                let coordinate = crate::Coordinate {
                    x: pin.x,
                    y: rows.checked_add(u32::from(offset))?,
                };
                return Some(match tag {
                    crate::point::Tag::Active => crate::Point::Active(coordinate),
                    crate::point::Tag::Viewport => crate::Point::Viewport(coordinate),
                    crate::point::Tag::Screen => crate::Point::Screen(coordinate),
                    crate::point::Tag::History => crate::Point::History(coordinate),
                });
            }
            rows = rows.checked_add(u32::from(page.size.rows - start))?;
            current = self.nodes.get(key)?.next;
        }
        None
    }

    /// Borrow the row and packed cell selected by a region-relative point.
    #[must_use]
    pub fn get_cell(&self, point: crate::Point) -> Option<Cell<'_>> {
        let pin = self.pin(point)?;
        let (row, cell) = pin.row_and_cell(self)?;
        Some(Cell {
            node: pin.node,
            row,
            cell,
            row_idx: pin.y,
            col_idx: pin.x,
        })
    }
}
