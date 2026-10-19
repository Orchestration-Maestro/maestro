//! Row removal through row-owned storage rotation and managed boundary copies.

use super::{Direction, NodeId, PageList, Pin};
use crate::{Coordinate, Point, page::StorageError};

impl PageList {
    /// Remove one row, shifting later rows up and leaving a blank final row.
    ///
    /// # Errors
    /// Returns a managed boundary-copy error; preceding rotations remain applied.
    pub fn erase_row(&mut self, point: Point) -> Result<(), StorageError> {
        self.shift_row(point, None)
    }

    /// Remove one row and shift at most `limit` following rows into its place.
    ///
    /// # Errors
    /// Returns a managed boundary-copy error; preceding rotations remain applied.
    pub fn erase_row_bounded(&mut self, point: Point, limit: usize) -> Result<(), StorageError> {
        self.shift_row(point, Some(limit))
    }

    /// Rotate each affected page once, cloning only rows crossing page boundaries.
    fn shift_row(&mut self, point: Point, limit: Option<usize>) -> Result<(), StorageError> {
        let Some(pin) = self.pin(point) else {
            return Ok(());
        };
        self.viewport_pin_row_offset = None;
        let mut node = pin.node;
        let mut start = pin.y;
        let mut remaining = limit.unwrap_or(usize::MAX);
        let mut previous = None;
        loop {
            let Some(page) = self.page(node) else {
                return Ok(());
            };
            let end = usize::from(page.size.rows).min(
                usize::from(start)
                    .saturating_add(remaining)
                    .saturating_add(1),
            );
            let end = u16::try_from(end).unwrap_or(page.size.rows);
            let final_page = end < page.size.rows
                || self.nodes.get(node).is_none_or(|n| n.next.is_none())
                || usize::from(end - start) > remaining;
            if final_page && let Some(page) = self.page_mut(node) {
                page.clear_cells(start, 0..page.size.cols);
            }
            if let Some(page) = self.page_mut(node) {
                page.rotate_rows(start..end, 1);
                page.dirty = true;
            }
            self.shift_pins(node, start..end, previous, limit.is_some());
            if final_page {
                break;
            }
            remaining = remaining.saturating_sub(usize::from(end - start));
            let Some(next) = self.nodes.get(node).and_then(|n| n.next) else {
                break;
            };
            if let Some((destination, source)) = self.nodes.get_pair_mut(node, next) {
                destination.page.clone_row_from(&source.page, end - 1, 0)?;
            }
            previous = Some((node, end - 1));
            node = next;
            start = 0;
        }
        self.fixup_viewport(0);
        Ok(())
    }

    /// Remap retained row ownership through one rotated region.
    fn shift_pins(
        &mut self,
        node: NodeId,
        rows: std::ops::Range<u16>,
        previous: Option<(NodeId, u16)>,
        bounded: bool,
    ) {
        for pin in self
            .tracked
            .values_mut()
            .filter(|p| p.node == node && rows.contains(&p.y))
        {
            if pin.y == 0
                && let Some((node, y)) = previous
            {
                pin.node = node;
                pin.y = y;
                continue;
            }
            if pin.y == rows.start && !bounded {
                continue;
            }
            if pin.y == 0 {
                pin.x = 0;
            } else {
                pin.y -= 1;
            }
        }
    }

    /// Remove the historical prefix through an optional inclusive endpoint.
    pub fn erase_history(&mut self, bottom: Option<Point>) {
        self.erase_rows(Point::History(Coordinate::default()), bottom);
    }

    /// Remove active rows through `y` inclusive, restoring the active height.
    ///
    /// # Panics
    /// Panics when `y` is outside the active area.
    pub fn erase_active(&mut self, y: u16) {
        assert!(y < self.rows());
        self.erase_rows(
            Point::Active(Coordinate::default()),
            Some(Point::Active(Coordinate {
                x: 0,
                y: u32::from(y),
            })),
        );
    }

    /// Remove boundary chunks, retaining at least one resident page.
    fn erase_rows(&mut self, start: Point, bottom: Option<Point>) {
        let mut iterator = self.page_iterator(Direction::RightDown, start, bottom);
        let mut erased = 0;
        self.viewport_pin_row_offset = None;
        while let Some(chunk) = iterator.next(self) {
            let count = chunk.end - chunk.start;
            if chunk.full_page(self) && self.first != self.last {
                erased += usize::from(count);
                self.erase_page(chunk.node);
                continue;
            }
            if chunk.full_page(self)
                && let Some(page) = self.page_mut(chunk.node)
            {
                page.reinit();
                page.size.rows = 0;
            } else {
                self.erase_page_rows(chunk);
            }
            erased += usize::from(count);
        }
        self.total_rows -= erased;
        if start.tag() == crate::point::Tag::Active {
            for _ in 0..erased {
                self.grow();
            }
        }
        self.fixup_viewport(erased);
    }

    /// Remove selected rows without copying cells or page-owned managed tables.
    fn erase_page_rows(&mut self, chunk: super::Chunk) {
        let count = chunk.end - chunk.start;
        let Some(page) = self.page_mut(chunk.node) else {
            return;
        };
        page.rotate_rows(chunk.start..page.size.rows, usize::from(count));
        let end = page.size.rows - count;
        for y in end..page.size.rows {
            page.clear_cells(y, 0..page.size.cols);
        }
        for y in chunk.start..end {
            page.get_row_mut(y).set_dirty(true);
        }
        page.size.rows = end;
        for pin in self
            .tracked
            .values_mut()
            .filter(|p| p.node == chunk.node && p.y >= chunk.start)
        {
            if pin.y >= chunk.end {
                pin.y -= count;
            } else {
                pin.y = chunk.start;
                pin.x = 0;
            }
        }
    }

    /// Release a boundary page and move retained positions to its neighbour.
    pub(super) fn erase_page(&mut self, key: NodeId) {
        let Some(node) = self.nodes.remove(key) else {
            return;
        };
        let Some(neighbour) = node.prev.or(node.next) else {
            return;
        };
        for pin in self.tracked.values_mut().filter(|pin| pin.node == key) {
            *pin = Pin {
                node: neighbour,
                x: 0,
                y: 0,
                garbage: pin.garbage,
            };
        }
        if let Some(prev) = node.prev.and_then(|key| self.nodes.get_mut(key)) {
            prev.next = node.next;
        } else if let Some(next) = node.next {
            self.first = next;
        }
        if let Some(next) = node.next.and_then(|key| self.nodes.get_mut(key)) {
            next.prev = node.prev;
        } else if let Some(prev) = node.prev {
            self.last = prev;
        }
        self.page_size -= Self::page_charge(node.page.capacity);
    }
}
