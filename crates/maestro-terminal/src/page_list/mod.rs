//! Ordered terminal pages with retained cell positions.

mod arena;
mod erase;
mod resize;
pub use resize::{Cursor, Resize};
mod grow;
mod reflow;
mod reflow_cursor;
mod reflow_transfer;
pub use grow::{IncreaseCapacity, IncreaseCapacityError};
mod scroll;
pub use scroll::{Scroll, Scrollbar, Viewport};
mod split;
pub use split::SplitError;
mod iter;
mod pin;
mod pin_movement;
pub use pin_movement::Offset;
mod prompt;
pub use prompt::PromptIterator;
#[cfg(test)]
mod tests;
pub use iter::{CellIterator, Chunk, Direction, PageIterator, RowIterator};
pub use pin::{Cell, CellSubset, Pin, TrackedPin};

use crate::{
    Page, Point,
    page::layout::{Adjustment, Capacity, STD_CAPACITY},
};
use arena::Arena;
use std::num::NonZeroU16;

/// A page lifetime belonging to one list-owned table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    /// Identity of the owning table.
    arena: u64,
    /// Index of the reusable slot.
    slot: usize,
    /// Lifetime of the slot's current value.
    generation: u64,
}

/// A resident page and its immediate ordered neighbours.
struct Node {
    /// The actual managed storage, never a forwarding copy.
    page: Page,
    /// Earlier page, absent at the oldest retained page.
    prev: Option<NodeId>,
    /// Later page, absent at the active bottom.
    next: Option<NodeId>,
}

/// Page ownership, active dimensions and retained positions for one screen.
pub struct PageList {
    /// Resident pages indexed by lifetime.
    nodes: Arena<Node>,
    /// Oldest retained page.
    first: NodeId,
    /// Last page in display order.
    last: NodeId,
    /// List-owned retained cell locations.
    tracked: Arena<Pin>,
    /// Reserved position used when the viewport is pinned.
    viewport_pin: TrackedPin,
    /// How the visible origin follows retained content.
    viewport: Viewport,
    /// Known absolute row of the reserved viewport position.
    viewport_pin_row_offset: Option<usize>,
    /// Desired column count.
    cols: NonZeroU16,
    /// Active height.
    rows: NonZeroU16,
    /// Cached addressable row count.
    total_rows: usize,
    /// Current logical page-byte charge.
    page_size: u64,
    /// Caller-supplied history limit, absent for unbounded history.
    explicit_max_size: Option<usize>,
    /// Minimum byte budget needed to retain the active area.
    min_max_size: u64,
}

impl PageList {
    /// Create zeroed active storage and reserve the viewport's tracked position.
    #[must_use]
    pub fn init(cols: NonZeroU16, rows: NonZeroU16, max_size: Option<usize>) -> Self {
        let capacity = Self::initial_capacity(cols.get());
        let mut nodes = Arena::new();
        let mut page = Page::init(capacity);
        page.size.rows = rows.get().min(capacity.rows);
        let mut total_rows = usize::from(page.size.rows);
        let first = nodes.insert(Node {
            page,
            prev: None,
            next: None,
        });
        let mut last = first;
        while total_rows < usize::from(rows.get()) {
            let mut page = Page::init(capacity);
            page.size.rows = u16::try_from(usize::from(rows.get()) - total_rows)
                .unwrap_or(u16::MAX)
                .min(capacity.rows);
            total_rows += usize::from(page.size.rows);
            let next = nodes.insert(Node {
                page,
                prev: Some(last),
                next: None,
            });
            if let Some(node) = nodes.get_mut(last) {
                node.next = Some(next);
            }
            last = next;
        }
        let page_count = usize::from(rows.get()).div_ceil(usize::from(capacity.rows));
        let mut tracked = Arena::new();
        let viewport_pin = TrackedPin(tracked.insert(Pin {
            node: first,
            x: 0,
            y: 0,
            garbage: false,
        }));
        Self {
            nodes,
            first,
            last,
            tracked,
            viewport_pin,
            viewport: Viewport::Active,
            viewport_pin_row_offset: None,
            cols,
            rows,
            total_rows,
            page_size: Self::page_charge(capacity) * u64::try_from(page_count).unwrap_or(u64::MAX),
            explicit_max_size: max_size,
            min_max_size: Self::standard_bytes()
                * u64::try_from(page_count + 1).unwrap_or(u64::MAX),
        }
    }

    /// Clear content and historical pages, retaining handles at the new origin.
    pub fn reset(&mut self) {
        let mut replacement = Self::init(self.cols, self.rows, self.explicit_max_size);
        std::mem::swap(&mut replacement.tracked, &mut self.tracked);
        replacement.viewport_pin = self.viewport_pin;
        for pin in replacement.tracked.values_mut() {
            *pin = Pin {
                node: replacement.first,
                x: 0,
                y: 0,
                garbage: true,
            };
        }
        if let Some(pin) = replacement.tracked.get_mut(replacement.viewport_pin.0) {
            pin.garbage = false;
        }
        *self = replacement;
    }

    /// Fit columns within a standard page, growing the page if one row cannot fit.
    fn initial_capacity(cols: u16) -> Capacity {
        STD_CAPACITY
            .adjust(Adjustment { cols: Some(cols) })
            .unwrap_or(Capacity {
                cols,
                ..STD_CAPACITY
            })
    }

    /// Standard logical page allocation size.
    fn standard_bytes() -> u64 {
        Page::layout(STD_CAPACITY).total_size
    }

    /// Small page layouts retain the full standard allocation charge.
    fn page_charge(capacity: Capacity) -> u64 {
        Page::layout(capacity)
            .total_size
            .max(Self::standard_bytes())
    }

    /// Read the desired column count.
    #[must_use]
    pub fn cols(&self) -> u16 {
        self.cols.get()
    }

    /// Read the active row count.
    #[must_use]
    pub fn rows(&self) -> u16 {
        self.rows.get()
    }

    /// Effective byte budget; the active-area minimum takes priority.
    #[must_use]
    pub fn max_size(&self) -> u64 {
        self.explicit_max_size
            .map_or(u64::MAX, |v| u64::try_from(v).unwrap_or(u64::MAX))
            .max(self.min_max_size)
    }

    /// Logical bytes charged for all resident pages.
    #[must_use]
    pub fn charged_bytes(&self) -> u64 {
        self.page_size
    }

    /// Number of addressable rows, including history.
    #[must_use]
    pub fn total_rows(&self) -> usize {
        self.total_rows
    }

    /// Number of rows preceding the editable area.
    #[must_use]
    pub fn history_rows(&self) -> usize {
        self.total_rows.saturating_sub(usize::from(self.rows()))
    }

    /// Number of retained page lifetimes.
    #[must_use]
    pub fn total_pages(&self) -> usize {
        self.nodes.iter().count()
    }

    /// Borrow the actual page for a current key.
    #[must_use]
    pub fn page(&self, node: NodeId) -> Option<&Page> {
        Some(&self.nodes.get(node)?.page)
    }

    /// Mutably borrow the actual page for a current key.
    pub fn page_mut(&mut self, node: NodeId) -> Option<&mut Page> {
        Some(&mut self.nodes.get_mut(node)?.page)
    }

    /// Check whether a page key still belongs to this live list.
    #[must_use]
    pub fn node_is_valid(&self, node: NodeId) -> bool {
        self.nodes.get(node).is_some()
    }

    /// Check the lifetime and current bounds of a cell location.
    #[must_use]
    pub fn pin_is_valid(&self, pin: Pin) -> bool {
        self.page(pin.node)
            .is_some_and(|page| pin.x < page.size.cols && pin.y < page.size.rows)
    }

    /// Resolve a coordinate relative to its region's origin.
    #[must_use]
    pub fn pin(&self, point: Point) -> Option<Pin> {
        let coord = point.coord();
        if coord.x >= self.cols() {
            return None;
        }
        let tag = point.tag();
        let mut pin = self
            .get_top_left(tag)
            .down(self, usize::try_from(coord.y).ok()?)?;
        pin.x = coord.x;
        self.pin_is_valid(pin).then_some(pin)
    }

    /// Resolve the final cell in a region, or no cell for empty history.
    #[must_use]
    pub fn get_bottom_right(&self, tag: crate::point::Tag) -> Option<Pin> {
        if tag == crate::point::Tag::Viewport {
            let mut pin = self
                .viewport_origin()
                .down(self, usize::from(self.rows() - 1))?;
            pin.x = self.page(pin.node)?.size.cols - 1;
            return Some(pin);
        }
        if tag == crate::point::Tag::History {
            let row = self.history_rows().checked_sub(1)?;
            let mut pin = self.pin_at_row(row, 0)?;
            pin.x = self.page(pin.node)?.size.cols - 1;
            return Some(pin);
        }
        let page = self.page(self.last)?;
        Some(Pin {
            node: self.last,
            x: page.size.cols - 1,
            y: page.size.rows - 1,
            garbage: false,
        })
    }

    /// Locate an absolute row using linked page order.
    fn pin_at_row(&self, mut row: usize, x: u16) -> Option<Pin> {
        let mut current = Some(self.first);
        while let Some(key) = current {
            let node = self.nodes.get(key)?;
            if row < usize::from(node.page.size.rows) {
                return (x < node.page.size.cols).then_some(Pin {
                    node: key,
                    x,
                    y: u16::try_from(row).ok()?,
                    garbage: false,
                });
            }
            row -= usize::from(node.page.size.rows);
            current = node.next;
        }
        None
    }

    /// Register a position until explicit release.
    pub fn track_pin(&mut self, pin: Pin) -> Option<TrackedPin> {
        self.pin_is_valid(pin)
            .then(|| TrackedPin(self.tracked.insert(pin)))
    }

    /// Release a retained position without releasing the reserved viewport slot.
    pub fn untrack_pin(&mut self, pin: TrackedPin) {
        if pin != self.viewport_pin {
            self.tracked.remove(pin.0);
        }
    }

    /// Read a live retained position.
    #[must_use]
    pub fn tracked_pin(&self, pin: TrackedPin) -> Option<&Pin> {
        self.tracked.get(pin.0)
    }

    /// Edit a live retained position.
    pub fn tracked_pin_mut(&mut self, pin: TrackedPin) -> Option<&mut Pin> {
        self.tracked.get_mut(pin.0)
    }

    /// Visit all registered positions, including the reserved viewport slot.
    pub fn tracked_pins(&self) -> impl Iterator<Item = (TrackedPin, &Pin)> + '_ {
        self.tracked.iter().map(|(id, pin)| (TrackedPin(id), pin))
    }

    /// Count registered positions, including the reserved viewport slot.
    #[must_use]
    pub fn count_tracked_pins(&self) -> usize {
        self.tracked.iter().count()
    }
}
