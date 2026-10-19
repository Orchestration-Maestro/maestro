//! Owned traversal cursors borrowing pages only while each item is requested.

use super::{NodeId, PageList, Pin};
use crate::{Point, page::row::Row};

/// Direction through ordered terminal positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Earlier columns and rows.
    LeftUp,
    /// Later columns and rows.
    RightDown,
}

/// A half-open range of rows in one page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Owning page lifetime.
    pub node: NodeId,
    /// Inclusive first page-local row.
    pub start: u16,
    /// Exclusive final page-local row.
    pub end: u16,
}

/// Cursor through contiguous row chunks, without retaining a page borrow.
pub struct PageIterator {
    /// Next row to visit.
    row: Option<Pin>,
    /// Inclusive endpoint, absent for unbounded traversal.
    limit: Option<Pin>,
    /// Traversal direction.
    direction: Direction,
}

impl PageIterator {
    /// Return the next half-open page chunk and advance in linked order.
    pub fn next(&mut self, list: &PageList) -> Option<Chunk> {
        let row = self.row.take()?;
        let node = list.nodes.get(row.node)?;
        if !list.pin_is_valid(row) {
            return None;
        }
        let same_page = self.limit.filter(|limit| limit.node == row.node);
        let (start, end, next) = match self.direction {
            Direction::RightDown => (
                row.y,
                same_page.map_or(node.page.size.rows, |limit| limit.y.saturating_add(1)),
                node.next.map(|node| Pin {
                    node,
                    x: 0,
                    y: 0,
                    garbage: false,
                }),
            ),
            Direction::LeftUp => (
                same_page.map_or(0, |limit| limit.y),
                row.y + 1,
                node.prev.and_then(|key| {
                    Some(Pin {
                        node: key,
                        x: 0,
                        y: list.page(key)?.size.rows - 1,
                        garbage: false,
                    })
                }),
            ),
        };
        if start >= end || end > node.page.size.rows {
            return None;
        }
        self.row = if same_page.is_some() { None } else { next };
        Some(Chunk {
            node: row.node,
            start,
            end,
        })
    }
}

impl Pin {
    /// Create an inclusive directional traversal starting at this row.
    #[must_use]
    pub fn page_iterator(self, direction: Direction, limit: Option<Self>) -> PageIterator {
        PageIterator {
            row: Some(self),
            limit,
            direction,
        }
    }
}

impl PageList {
    /// Traverse a region from its lower to upper endpoint, or in reverse.
    #[must_use]
    pub fn page_iterator(
        &self,
        direction: Direction,
        start: Point,
        end: Option<Point>,
    ) -> PageIterator {
        let low = self.pin(start);
        let tag = start.tag();
        let high = end.map_or_else(|| self.get_bottom_right(tag), |point| self.pin(point));
        let (row, limit) = match direction {
            Direction::RightDown => (low, high),
            Direction::LeftUp => (high, low),
        };
        PageIterator {
            row: row.filter(|_| limit.is_some()),
            limit,
            direction,
        }
    }
}

impl Chunk {
    /// Borrow the selected metadata while its page and range remain valid.
    #[must_use]
    pub fn rows(self, list: &PageList) -> Option<&[Row]> {
        let page = list.page(self.node)?;
        if self.start > self.end || self.end > page.size.rows {
            return None;
        }
        page.rows
            .get(usize::from(self.start)..usize::from(self.end))
    }

    /// Whether this chunk covers every addressable row of its current page.
    #[must_use]
    pub fn full_page(self, list: &PageList) -> bool {
        self.start == 0
            && list
                .page(self.node)
                .is_some_and(|page| self.end == page.size.rows)
    }

    /// Whether two half-open row ranges intersect in the same page lifetime.
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.node == other.node && self.start < other.end && self.end > other.start
    }
}

/// Cursor through individual rows with a call-scoped list borrow.
pub struct RowIterator {
    /// Shared linked-page traversal.
    page_it: PageIterator,
    /// Current contiguous chunk.
    chunk: Option<Chunk>,
    /// Next row within that chunk.
    offset: Option<u16>,
}

impl RowIterator {
    /// Return the next row at column zero.
    pub fn next(&mut self, list: &PageList) -> Option<Pin> {
        if self.chunk.is_none() {
            self.chunk = self.page_it.next(list);
        }
        let chunk = self.chunk?;
        let y = self.offset.unwrap_or(match self.page_it.direction {
            Direction::RightDown => chunk.start,
            Direction::LeftUp => chunk.end - 1,
        });
        let next = match self.page_it.direction {
            Direction::RightDown => y.checked_add(1).filter(|next| *next < chunk.end),
            Direction::LeftUp => y.checked_sub(1).filter(|next| *next >= chunk.start),
        };
        self.offset = next;
        if next.is_none() {
            self.chunk = None;
        }
        let pin = Pin {
            node: chunk.node,
            x: 0,
            y,
            garbage: false,
        };
        list.pin_is_valid(pin).then_some(pin)
    }
}

/// Cursor through row-major cells without retaining a page borrow.
pub struct CellIterator {
    /// Shared row traversal.
    row_it: RowIterator,
    /// Next cell in the current row.
    cell: Option<Pin>,
    /// Starting column, consumed only for the first row.
    initial_x: Option<u16>,
}

impl CellIterator {
    /// Return the next cell, crossing each row only once.
    pub fn next(&mut self, list: &PageList) -> Option<Pin> {
        let cell = if let Some(pin) = self.cell.take() {
            pin
        } else {
            let mut pin = self.row_it.next(list)?;
            pin.x = self
                .initial_x
                .take()
                .unwrap_or(match self.row_it.page_it.direction {
                    Direction::RightDown => 0,
                    Direction::LeftUp => list.page(pin.node)?.size.cols - 1,
                });
            pin
        };
        if !list.pin_is_valid(cell) {
            return None;
        }
        self.cell = match self.row_it.page_it.direction {
            Direction::RightDown => cell.right(list, 1),
            Direction::LeftUp => cell.x.checked_sub(1).map(|x| Pin { x, ..cell }),
        };
        Some(cell)
    }
}

impl PageIterator {
    /// Interpret page chunks as rows with lazy first-borrow initialization.
    fn into_rows(self) -> RowIterator {
        RowIterator {
            page_it: self,
            chunk: None,
            offset: None,
        }
    }
}

impl Pin {
    /// Traverse rows starting at this location, including the endpoint row.
    #[must_use]
    pub fn row_iterator(self, direction: Direction, limit: Option<Self>) -> RowIterator {
        self.page_iterator(direction, limit).into_rows()
    }

    /// Traverse cells starting at this column, including the endpoint row.
    #[must_use]
    pub fn cell_iterator(self, direction: Direction, limit: Option<Self>) -> CellIterator {
        CellIterator {
            row_it: self.row_iterator(direction, limit),
            cell: None,
            initial_x: Some(self.x),
        }
    }
}

impl PageList {
    /// Traverse the selected region's rows in the supplied direction.
    #[must_use]
    pub fn row_iterator(
        &self,
        direction: Direction,
        start: Point,
        end: Option<Point>,
    ) -> RowIterator {
        self.page_iterator(direction, start, end).into_rows()
    }

    /// Traverse the selected region's cells in the supplied direction.
    #[must_use]
    pub fn cell_iterator(
        &self,
        direction: Direction,
        start: Point,
        end: Option<Point>,
    ) -> CellIterator {
        let page_it = self.page_iterator(direction, start, end);
        let initial_x = page_it.row.map(|pin| pin.x);
        CellIterator {
            row_it: page_it.into_rows(),
            cell: None,
            initial_x,
        }
    }
}
