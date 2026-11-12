//! Append rows while retaining the active area under the logical byte limit.

use super::{Node, NodeId, PageList, Pin};
use crate::Page;

impl PageList {
    /// Append one row, returning its page key only when a new page is needed.
    ///
    /// Oldest pages are pruned before allocation only when another page exists,
    /// a standard allocation would exceed the budget, and the active area fits.
    pub fn grow(&mut self) -> Option<NodeId> {
        let last = self.nodes.get_mut(self.last)?;
        if last.page.size.rows < last.page.capacity.rows {
            last.page.size.rows += 1;
            self.total_rows += 1;
            return None;
        }
        let first = self.nodes.get(self.first)?;
        let remaining = self.total_rows - usize::from(first.page.size.rows);
        let removed = if self.first != self.last
            && self.page_size.saturating_add(Self::standard_bytes()) > self.max_size()
            && remaining + 1 >= usize::from(self.rows())
        {
            self.prune_first()
        } else {
            0
        };
        let capacity = Self::initial_capacity(self.cols());
        let mut page = Page::init(capacity);
        page.size.rows = 1;
        let next = self.nodes.insert(Node {
            page,
            prev: Some(self.last),
            next: None,
        });
        if let Some(last) = self.nodes.get_mut(self.last) {
            last.next = Some(next);
        }
        self.last = next;
        self.total_rows += 1;
        self.page_size += Self::page_charge(capacity);
        if removed > 0 {
            self.fixup_viewport(removed);
        }
        Some(next)
    }

    /// Remove the oldest page and remap lost retained content to the new origin.
    fn prune_first(&mut self) -> usize {
        let old = self.first;
        let Some(next) = self.nodes.get(old).and_then(|node| node.next) else {
            return 0;
        };
        let Some(node) = self.nodes.remove(old) else {
            return 0;
        };
        self.first = next;
        if let Some(first) = self.nodes.get_mut(next) {
            first.prev = None;
        }
        self.total_rows -= usize::from(node.page.size.rows);
        self.page_size -= Self::page_charge(node.page.capacity);
        for pin in self.tracked.values_mut().filter(|pin| pin.node == old) {
            *pin = Pin {
                node: next,
                x: 0,
                y: 0,
                garbage: true,
            };
        }
        if let Some(pin) = self.tracked.get_mut(self.viewport_pin.0) {
            pin.garbage = false;
        }
        usize::from(node.page.size.rows)
    }
}

/// Managed storage dimension to grow before retrying an insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncreaseCapacity {
    /// Deduplicated style entries.
    Styles,
    /// Cluster suffix storage.
    GraphemeBytes,
    /// Hyperlink identity and map storage.
    HyperlinkBytes,
    /// Hyperlink string storage.
    StringBytes,
}

/// A managed capacity cannot grow or the supplied node has expired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncreaseCapacityError {
    /// The dimension or logical page size cannot grow further.
    OutOfSpace,
    /// The node belongs to another list or an expired lifetime.
    InvalidNode,
}

impl IncreaseCapacity {
    /// Double one capacity, admitting the default when starting from zero.
    fn adjust(
        self,
        capacity: &mut crate::page::layout::Capacity,
    ) -> Result<(), IncreaseCapacityError> {
        use crate::page::layout::Capacity;
        /// Grow one logical dimension without integer overflow.
        fn increase(value: u32, default: u32, max: u32) -> Result<u32, IncreaseCapacityError> {
            if value == 0 {
                Ok(default)
            } else if value == max {
                Err(IncreaseCapacityError::OutOfSpace)
            } else {
                Ok(value.saturating_mul(2).min(max))
            }
        }
        let default = Capacity::default();
        match self {
            Self::Styles => {
                capacity.styles = u16::try_from(increase(
                    u32::from(capacity.styles),
                    u32::from(default.styles),
                    u32::from(u16::MAX),
                )?)
                .map_err(|_| IncreaseCapacityError::OutOfSpace)?;
            }
            Self::GraphemeBytes => {
                capacity.grapheme_bytes =
                    increase(capacity.grapheme_bytes, default.grapheme_bytes, u32::MAX)?;
            }
            Self::HyperlinkBytes => {
                capacity.hyperlink_bytes = u16::try_from(increase(
                    u32::from(capacity.hyperlink_bytes),
                    u32::from(default.hyperlink_bytes),
                    u32::from(u16::MAX),
                )?)
                .map_err(|_| IncreaseCapacityError::OutOfSpace)?;
            }
            Self::StringBytes => {
                capacity.string_bytes =
                    increase(capacity.string_bytes, default.string_bytes, u32::MAX)?;
            }
        }
        if Page::layout(*capacity).total_size > u64::from(u32::MAX) {
            return Err(IncreaseCapacityError::OutOfSpace);
        }
        Ok(())
    }
}

impl PageList {
    /// Rebuild a page at increased managed capacity and remap its retained pins.
    ///
    /// # Errors
    /// Returns `InvalidNode` for a stale or foreign key and `OutOfSpace` when
    /// the requested growth or managed content cannot fit the replacement.
    pub fn increase_capacity(
        &mut self,
        node: NodeId,
        adjustment: Option<IncreaseCapacity>,
    ) -> Result<NodeId, IncreaseCapacityError> {
        let old = self
            .nodes
            .get(node)
            .ok_or(IncreaseCapacityError::InvalidNode)?;
        let mut capacity = old.page.capacity;
        if let Some(adjustment) = adjustment {
            adjustment.adjust(&mut capacity)?;
        }
        let mut page = Page::init(capacity);
        page.size = old.page.size;
        page.clone_from(&old.page, 0..old.page.size.rows)
            .map_err(|_| IncreaseCapacityError::OutOfSpace)?;
        page.dirty = old.page.dirty;
        let (prev, next) = (old.prev, old.next);
        let old_charge = Self::page_charge(old.page.capacity);
        let new = self.nodes.insert(Node { page, prev, next });
        if let Some(prev) = prev.and_then(|key| self.nodes.get_mut(key)) {
            prev.next = Some(new);
        } else {
            self.first = new;
        }
        if let Some(next) = next.and_then(|key| self.nodes.get_mut(key)) {
            next.prev = Some(new);
        } else {
            self.last = new;
        }
        for pin in self.tracked.values_mut().filter(|pin| pin.node == node) {
            pin.node = new;
        }
        self.nodes.remove(node);
        self.page_size = self.page_size - old_charge + Self::page_charge(capacity);
        Ok(new)
    }
}
