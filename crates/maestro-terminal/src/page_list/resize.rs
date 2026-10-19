//! Dimension changes without changing logical line boundaries.

use super::{Node, NodeId, PageList, TrackedPin};
use crate::{
    Page,
    page::{
        StorageError,
        cell::{Cell, Wide},
        layout::{Adjustment, Capacity},
    },
};
use std::num::NonZeroU16;

/// Cursor location used to preserve the active region during dimension changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    /// Active column before resizing.
    pub x: u16,
    /// Active row before resizing.
    pub y: u16,
    /// Optional retained location, including meaningful right-side blanks.
    pub pin: Option<TrackedPin>,
}

/// Requested dimensions and line rewrapping policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resize {
    /// New column count, or the current width.
    pub cols: Option<NonZeroU16>,
    /// New active height, or the current height.
    pub rows: Option<NonZeroU16>,
    /// Rewrap soft-wrapped lines when columns change.
    pub reflow: bool,
    /// Cursor whose active-area position should be preserved.
    pub cursor: Option<Cursor>,
}

impl Default for Resize {
    fn default() -> Self {
        Self {
            cols: None,
            rows: None,
            reflow: true,
            cursor: None,
        }
    }
}

impl PageList {
    /// Change active dimensions, rewrapping soft lines by default.
    ///
    /// # Errors
    /// Returns a managed-copy storage refusal; preceding page changes remain applied.
    pub fn resize(&mut self, opts: Resize) -> Result<(), StorageError> {
        self.viewport_pin_row_offset = None;
        let old_min = self.min_max_size;
        let cols = opts.cols.unwrap_or(self.cols);
        let rows = opts.rows.unwrap_or(self.rows);
        let capacity = Self::initial_capacity(cols.get());
        self.min_max_size =
            Self::standard_bytes() * (u64::from(rows.get()).div_ceil(u64::from(capacity.rows)) + 1);
        let result = self.resize_ordered(opts);
        if result.is_err() {
            self.min_max_size = old_min;
        }
        result
    }

    /// Unwrap before growing height, but establish height before wrapping narrower.
    fn resize_ordered(&mut self, opts: Resize) -> Result<(), StorageError> {
        let cols = opts.cols.unwrap_or(self.cols);
        if !opts.reflow || cols == self.cols {
            return self.resize_without_reflow(opts);
        }
        if cols > self.cols {
            self.resize_columns(cols, opts.cursor)?;
            self.resize_without_reflow(opts)?;
        } else {
            self.resize_without_reflow(Resize { cols: None, ..opts })?;
            self.resize_columns(cols, opts.cursor)?;
        }
        self.resize_viewport();
        Ok(())
    }
    /// Resize columns in place or by page replacement, then adjust active height.
    pub(super) fn resize_without_reflow(&mut self, opts: Resize) -> Result<(), StorageError> {
        if let Some(cols) = opts.cols {
            self.resize_plain_columns(cols)?;
        }
        if let Some(rows) = opts.rows {
            self.resize_plain_rows(rows, opts.cursor);
        }
        Ok(())
    }

    /// Only a pinned viewport follows content into the newly active region.
    pub(super) fn resize_viewport(&mut self) {
        if self.viewport == super::Viewport::Pin
            && self.tracked_pin(self.viewport_pin).is_some_and(|pin| {
                self.point_from_pin(crate::point::Tag::Active, *pin)
                    .is_some()
            })
        {
            self.viewport = super::Viewport::Active;
        }
    }

    /// Change width without altering logical line boundaries.
    fn resize_plain_columns(&mut self, cols: NonZeroU16) -> Result<(), StorageError> {
        match cols.cmp(&self.cols) {
            std::cmp::Ordering::Less => self.shrink_columns(cols),
            std::cmp::Ordering::Greater => {
                let mut next = Some(self.first);
                while let Some(key) = next {
                    next = self.nodes.get(key).and_then(|node| node.next);
                    self.grow_columns(key, cols.get())?;
                }
                self.cols = cols;
            }
            std::cmp::Ordering::Equal => {}
        }
        Ok(())
    }

    /// Trim blanks on shrink; pull history down unless preserving a higher cursor.
    fn resize_plain_rows(&mut self, rows: NonZeroU16, cursor: Option<Cursor>) {
        if rows < self.rows {
            let trimmed = self.trim_trailing_blank_rows(self.rows() - rows.get());
            self.total_rows -= usize::from(trimmed);
            self.rows = rows;
            return;
        }
        if rows == self.rows {
            return;
        }
        let preserve_cursor = cursor.is_some_and(|cursor| cursor.y < self.rows() - 1);
        let growth = if preserve_cursor {
            usize::from(rows.get() - self.rows())
        } else {
            usize::from(rows.get()).saturating_sub(self.total_rows)
        };
        self.rows = rows;
        for _ in 0..growth {
            self.grow();
        }
        self.resize_viewport();
    }

    /// Release truncated managed cells before reducing the addressable width.
    fn shrink_columns(&mut self, cols: NonZeroU16) {
        let old_cols = self.cols();
        for node in self.nodes.values_mut() {
            for y in 0..node.page.size.rows {
                node.page.clear_cells(y, cols.get()..old_cols);
            }
            node.page.size.cols = cols.get();
        }
        for pin in self.tracked.values_mut() {
            pin.x = pin.x.min(cols.get() - 1);
        }
        self.cols = cols;
    }

    /// Remove trailing text-free rows, stopping at any retained row.
    fn trim_trailing_blank_rows(&mut self, max: u16) -> u16 {
        let mut trimmed = 0;
        while trimmed < max {
            let Some(page) = self.page(self.last) else {
                break;
            };
            let y = page.size.rows - 1;
            if Cell::has_text_any(page.get_cells(y))
                || self
                    .tracked
                    .iter()
                    .any(|(_, p)| p.node == self.last && p.y == y)
            {
                break;
            }
            if y == 0 && self.first != self.last {
                self.erase_page(self.last);
            } else if let Some(page) = self.page_mut(self.last) {
                page.clear_cells(y, 0..page.size.cols);
                page.size.rows -= 1;
            }
            trimmed += 1;
        }
        trimmed
    }

    /// Widen a page, preferring retained allocation when no spacer needs repair.
    fn grow_columns(&mut self, key: NodeId, cols: u16) -> Result<(), StorageError> {
        let source = self.nodes.get(key).ok_or(StorageError::OutOfMemory)?;
        let old_cols = source.page.size.cols;
        if source.page.capacity.cols >= cols
            && (0..source.page.size.rows).all(|y| {
                source.page.get_cells(y)[usize::from(old_cols - 1)].wide() != Wide::SpacerHead
            })
        {
            if let Some(page) = self.page_mut(key) {
                page.size.cols = cols;
            }
            return Ok(());
        }
        let cap = Self::resized_capacity(&source.page, cols);
        let rows = source.page.size.rows;
        let mut copied = self.fill_previous_page(key);
        let pages = Self::widened_pages(
            self.page(key).ok_or(StorageError::OutOfMemory)?,
            cap,
            copied,
        );
        let pages = match pages {
            Ok(pages) => pages,
            Err(error) => {
                self.undo_previous_fill(key, copied);
                return Err(error);
            }
        };
        for page in pages {
            let len = page.size.rows;
            let new = self.insert_before(key, page);
            self.remap_rows(key, copied..copied + len, new, 0);
            copied += len;
        }
        debug_assert_eq!(copied, rows);
        self.erase_page(key);
        Ok(())
    }

    /// Build replacement chunks, keeping the source linked until all copies fit.
    fn widened_pages(
        source: &Page,
        cap: Capacity,
        mut copied: u16,
    ) -> Result<Vec<Page>, StorageError> {
        let mut pages = Vec::new();
        while copied < source.size.rows {
            let page = Self::widened_chunk(source, cap, copied)?;
            copied += page.size.rows;
            pages.push(page);
        }
        Ok(pages)
    }

    /// Copy as many rows as fit, stopping a chunk at a managed refusal.
    fn widened_chunk(source: &Page, cap: Capacity, copied: u16) -> Result<Page, StorageError> {
        let mut page = Page::init(cap);
        page.size.rows = 0;
        while page.size.rows < cap.rows && copied + page.size.rows < source.size.rows {
            let y = page.size.rows;
            page.size.rows += 1;
            let Err(error) = page.clone_row_from(source, y, copied + y) else {
                continue;
            };
            page.clear_cells(y, 0..cap.cols);
            page.size.rows -= 1;
            if y == 0 {
                return Err(error);
            }
            break;
        }
        Ok(page)
    }

    /// Undo only the preceding page's speculative fill when replacement fails.
    fn undo_previous_fill(&mut self, source: NodeId, copied: u16) {
        let Some(previous) = self.nodes.get(source).and_then(|n| n.prev) else {
            return;
        };
        let Some(page) = self.page_mut(previous) else {
            return;
        };
        let start = page.size.rows - copied;
        for y in start..page.size.rows {
            page.clear_cells(y, 0..page.size.cols);
        }
        page.size.rows = start;
        for pin in self
            .tracked
            .values_mut()
            .filter(|p| p.node == previous && p.y >= start)
        {
            pin.node = source;
            pin.y -= start;
        }
    }

    /// Fit the wider grid within the source allocation, retaining managed budgets.
    pub(super) fn resized_capacity(page: &Page, cols: u16) -> Capacity {
        page.capacity
            .adjust(Adjustment { cols: Some(cols) })
            .unwrap_or(Capacity {
                cols,
                rows: page.size.rows.min(page.capacity.rows),
                ..page.capacity
            })
    }

    /// Fill slack in the preceding resized page before allocating another page.
    fn fill_previous_page(&mut self, key: NodeId) -> u16 {
        let Some(previous) = self.nodes.get(key).and_then(|n| n.prev) else {
            return 0;
        };
        let Some((destination, source)) = self.nodes.get_pair_mut(previous, key) else {
            return 0;
        };
        let start = destination.page.size.rows;
        let limit = (destination.page.capacity.rows - start).min(source.page.size.rows);
        let mut copied = 0;
        while copied < limit {
            destination.page.size.rows += 1;
            if destination
                .page
                .clone_row_from(&source.page, start + copied, copied)
                .is_err()
            {
                destination
                    .page
                    .clear_cells(start + copied, 0..destination.page.size.cols);
                destination.page.size.rows -= 1;
                break;
            }
            copied += 1;
        }
        self.remap_rows(key, 0..copied, previous, start);
        copied
    }

    /// Link a replacement immediately before its source without changing row counts.
    fn insert_before(&mut self, key: NodeId, page: Page) -> NodeId {
        let previous = self.nodes.get(key).and_then(|n| n.prev);
        self.page_size += Self::page_charge(page.capacity);
        let new = self.nodes.insert(Node {
            page,
            prev: previous,
            next: Some(key),
        });
        if let Some(previous) = previous.and_then(|p| self.nodes.get_mut(p)) {
            previous.next = Some(new);
        } else {
            self.first = new;
        }
        if let Some(source) = self.nodes.get_mut(key) {
            source.prev = Some(new);
        }
        new
    }

    /// Move retained positions along with a contiguous copied row range.
    fn remap_rows(
        &mut self,
        source: NodeId,
        rows: std::ops::Range<u16>,
        target: NodeId,
        start: u16,
    ) {
        for pin in self
            .tracked
            .values_mut()
            .filter(|p| p.node == source && rows.contains(&p.y))
        {
            pin.node = target;
            pin.y = start + pin.y - rows.start;
        }
    }
}
