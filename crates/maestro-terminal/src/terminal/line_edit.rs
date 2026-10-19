//! Scrolling-region row shifts and insertion of background-preserving blanks.

use super::Terminal;
use crate::{
    Coordinate, Point,
    page::{
        CellPosition, StorageError,
        cell::{Content, Wide},
    },
    page_list::{IncreaseCapacity, IncreaseCapacityError as Error, Pin, TrackedPin},
};

impl Terminal {
    /// Insert blank cells at the cursor within the horizontal margins.
    pub fn insert_blanks(&mut self, count: usize) {
        let screen = self.screens.active_mut();
        screen.cursor.pending_wrap = false;
        let x = screen.cursor.x;
        let right = self.scrolling_region.right;
        if count == 0 || x < self.scrolling_region.left || x > right {
            return;
        }
        let Some(pin) = screen.pages.tracked_pin(screen.cursor.page_pin).copied() else {
            return;
        };
        if pin
            .row_and_cell(&screen.pages)
            .is_some_and(|(_, c)| c.wide() == Wide::SpacerTail)
        {
            screen.clear_cells(pin.node, pin.y, x - 1..x + 1);
        }
        if screen
            .pages
            .page(pin.node)
            .is_some_and(|p| p.get_cells(pin.y)[usize::from(right)].wide() == Wide::Wide)
        {
            screen.clear_cells(pin.node, pin.y, right..right + 2);
        }
        let count = u16::try_from(count.min(usize::from(right - x + 1))).unwrap_or(right - x + 1);
        let end = right + 1 - count;
        if end > x
            && screen
                .pages
                .page(pin.node)
                .is_some_and(|p| p.get_cells(pin.y)[usize::from(end - 1)].wide() == Wide::Wide)
        {
            screen.clear_cells(pin.node, pin.y, end - 1..end + 1);
        }
        if end > x
            && let Some(page) = screen.pages.page_mut(pin.node)
        {
            for col in (x..end).rev() {
                page.swap_cells(
                    CellPosition { x: col, y: pin.y },
                    CellPosition {
                        x: col + count,
                        y: pin.y,
                    },
                );
            }
        }
        screen.clear_cells(pin.node, pin.y, x..x + count);
        screen.cursor_mark_dirty();
    }

    /// Shift rows down, discarding rows below the bottom margin.
    ///
    /// # Errors
    /// Returns a capacity error if managed data cannot fit a destination page.
    pub fn insert_lines(&mut self, count: usize) -> Result<(), Error> {
        self.shift_lines(count, true)
    }

    /// Shift rows up, filling the bottom with background-preserving blanks.
    ///
    /// # Errors
    /// Returns a capacity error if managed data cannot fit a destination page.
    pub fn delete_lines(&mut self, count: usize) -> Result<(), Error> {
        self.shift_lines(count, false)
    }

    /// Traverse the selected region in overwrite-safe order through one row owner.
    fn shift_lines(&mut self, count: usize, insert: bool) -> Result<(), Error> {
        let r = self.scrolling_region;
        let c = &self.screens.active().cursor;
        let start = c.y;
        if count == 0 || start < r.top || start > r.bottom || c.x < r.left || c.x > r.right {
            return Ok(());
        }
        let count = u16::try_from(count.min(usize::from(r.bottom - start + 1)))
            .unwrap_or(r.bottom - start + 1);
        let result = self.shift_rows(start, count, insert);
        let screen = self.screens.active_mut();
        screen.cursor_absolute(r.left, start);
        screen.cursor.pending_wrap = false;
        result
    }

    /// Visit rows in the direction that preserves every source until consumed.
    fn shift_rows(&mut self, start: u16, count: u16, insert: bool) -> Result<(), Error> {
        let bottom = self.scrolling_region.bottom;
        let pin = self.row_pin(if insert { bottom } else { start })?;
        let tracked = self
            .screens
            .active_mut()
            .pages
            .track_pin(pin)
            .ok_or(Error::InvalidNode)?;
        let result = self.shift_pinned_rows(tracked, bottom - start, count, insert);
        self.screens.active_mut().pages.untrack_pin(tracked);
        result
    }

    /// Advance a retained destination monotonically, including after page growth.
    fn shift_pinned_rows(
        &mut self,
        tracked: TrackedPin,
        distance: u16,
        count: u16,
        insert: bool,
    ) -> Result<(), Error> {
        let advance = if insert { Pin::up } else { Pin::down };
        for offset in 0..=distance {
            let dst = self.shift_pin(tracked)?;
            let source = if offset < distance + 1 - count {
                let pages = &self.screens.active().pages;
                Some(advance(dst, pages, usize::from(count)).ok_or(Error::InvalidNode)?)
            } else {
                None
            };
            self.shift_row(tracked, source)?;
            if offset < distance {
                let dst = self.shift_pin(tracked)?;
                let pages = &mut self.screens.active_mut().pages;
                let next = advance(dst, pages, 1).ok_or(Error::InvalidNode)?;
                *pages.tracked_pin_mut(tracked).ok_or(Error::InvalidNode)? = next;
            }
        }
        Ok(())
    }

    /// Resolve a traversal handle after any destination page replacement.
    fn shift_pin(&self, tracked: TrackedPin) -> Result<Pin, Error> {
        self.screens
            .active()
            .pages
            .tracked_pin(tracked)
            .copied()
            .ok_or(Error::InvalidNode)
    }

    /// Prepare and move one row, retrying after destination capacity growth.
    fn shift_row(&mut self, destination: TrackedPin, source: Option<Pin>) -> Result<(), Error> {
        let dst = self.shift_pin(destination)?;
        self.row_will_be_shifted(dst)?;
        let r = self.scrolling_region;
        if let Some(source) = source {
            self.row_will_be_shifted(source)?;
            self.transfer_row(source, dst)?;
        } else {
            self.screens
                .active_mut()
                .clear_cells(dst.node, dst.y, r.left..r.right + 1);
        }
        self.shift_pin(destination)?
            .mark_dirty(&mut self.screens.active_mut().pages);
        Ok(())
    }

    /// Retry only the current cross-page transfer after growing its exhausted owner.
    fn transfer_row(&mut self, src: Pin, mut dst: Pin) -> Result<(), Error> {
        let r = self.scrolling_region;
        loop {
            let Err(error) =
                self.screens
                    .active_mut()
                    .pages
                    .transfer_row(src, dst, r.left..r.right + 1)
            else {
                return Ok(());
            };
            dst.node = self
                .screens
                .active_mut()
                .increase_capacity(dst.node, Some(copy_adjustment(error)))?;
        }
    }

    /// Resolve an active row without retaining a stale page lifetime across growth.
    fn row_pin(&self, y: u16) -> Result<Pin, Error> {
        self.screens
            .active()
            .pages
            .pin(Point::Active(Coordinate {
                x: 0,
                y: u32::from(y),
            }))
            .ok_or(Error::InvalidNode)
    }

    /// Remove split-wide boundaries and full-width wrap metadata before shifting.
    fn row_will_be_shifted(&mut self, pin: Pin) -> Result<(), Error> {
        let r = self.scrolling_region;
        let page = self
            .screens
            .active_mut()
            .pages
            .page_mut(pin.node)
            .ok_or(Error::InvalidNode)?;
        let last = page.size.cols - 1;
        if (r.right == self.cols - 1 || r.left < 2)
            && page.get_cells(pin.y)[usize::from(last)].wide() == Wide::SpacerHead
        {
            page.get_cells_mut(pin.y)[usize::from(last)].set_wide(Wide::Narrow);
        }
        let left_tail = page.get_cells(pin.y)[usize::from(r.left)].wide() == Wide::SpacerTail;
        let right_wide = page.get_cells(pin.y)[usize::from(r.right)].wide() == Wide::Wide;
        for x in [left_tail.then(|| r.left - 1), right_wide.then_some(r.right)]
            .into_iter()
            .flatten()
        {
            if page.get_cells(pin.y)[usize::from(x)].has_grapheme() {
                page.clear_grapheme(CellPosition { x, y: pin.y });
                page.update_row_grapheme_flag(pin.y);
            }
            let cells = page.get_cells_mut(pin.y);
            cells[usize::from(x)].set_content(Content::Codepoint('\0'));
            cells[usize::from(x)].set_wide(Wide::Narrow);
            cells[usize::from(x + 1)].set_wide(Wide::Narrow);
        }
        if r.left == 0 && r.right == self.cols - 1 {
            page.get_row_mut(pin.y).set_wrap(false);
            page.get_row_mut(pin.y).set_wrap_continuation(false);
        }
        Ok(())
    }

    /// Scroll the region down without changing cursor position or pending wrap.
    ///
    /// # Errors
    /// Returns a capacity error when a managed row transfer cannot fit.
    pub fn scroll_down(&mut self, count: usize) -> Result<(), Error> {
        self.scroll_region(count, true)
    }

    /// Scroll the region up, retaining history where the region permits it.
    ///
    /// # Errors
    /// Returns a capacity error when scrolling or managed transfer cannot fit.
    pub fn scroll_up(&mut self, count: usize) -> Result<(), Error> {
        self.scroll_region(count, false)
    }

    /// Preserve cursor state across either scrolling direction.
    fn scroll_region(&mut self, count: usize, down: bool) -> Result<(), Error> {
        let c = &self.screens.active().cursor;
        let (x, y, wrap) = (c.x, c.y, c.pending_wrap);
        let r = self.scrolling_region;
        let history = !down
            && r.top == 0
            && r.left == 0
            && r.right == self.cols - 1
            && (!self.screens.active().no_scrollback || r.bottom == self.rows - 1);
        let screen = self.screens.active_mut();
        screen.cursor_absolute(r.left, if history { r.bottom } else { r.top });
        let result = if history {
            (0..count.min(usize::from(r.bottom + 1))).try_for_each(|_| {
                self.screens
                    .active_mut()
                    .cursor_scroll_above()
                    .map_err(|_| Error::OutOfSpace)
            })
        } else {
            self.shift_lines(count, down)
        };
        let screen = self.screens.active_mut();
        screen.cursor_absolute(x, y);
        screen.cursor.pending_wrap = wrap;
        result
    }
}

/// Map a managed row-copy refusal to its existing page growth dimension.
fn copy_adjustment(error: StorageError) -> IncreaseCapacity {
    match error {
        StorageError::StyleSetOutOfMemory => IncreaseCapacity::Styles,
        StorageError::StringAllocOutOfMemory | StorageError::StringsOutOfMemory => {
            IncreaseCapacity::StringBytes
        }
        StorageError::GraphemeMapOutOfMemory | StorageError::GraphemeAllocOutOfMemory => {
            IncreaseCapacity::GraphemeBytes
        }
        _ => IncreaseCapacity::HyperlinkBytes,
    }
}
