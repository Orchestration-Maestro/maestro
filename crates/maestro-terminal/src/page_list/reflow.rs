//! Monotonic source-row consumption into a streaming destination cursor.

use super::{
    Cursor, Direction, Node, NodeId, PageList, Pin, TrackedPin, reflow_cursor::ReflowCursor,
    reflow_transfer::Written,
};
use crate::{
    Page,
    page::{
        CellPosition, StorageError,
        layout::{Adjustment, STD_CAPACITY},
        row::SemanticPrompt,
    },
    point::Tag,
};
use std::num::NonZeroU16;

/// Cursor ownership and active rows retained below its logical location.
struct PreservedCursor {
    /// Cursor position to follow through copying.
    tracked: TrackedPin,
    /// Whether this resize allocated the retained slot.
    owned: bool,
    /// Active rows originally beneath the cursor.
    remaining: u16,
    /// Continuation rows originally above the cursor.
    wrapped: usize,
}

impl PageList {
    /// Preserve cursor spacing while consuming each original page once.
    pub(super) fn resize_columns(
        &mut self,
        cols: NonZeroU16,
        cursor: Option<Cursor>,
    ) -> Result<(), StorageError> {
        let preserved = cursor.and_then(|cursor| self.preserve_cursor(cursor));
        let result = self.reflow_columns(cols, preserved.as_ref().map(|c| c.tracked));
        if result.is_ok()
            && let Some(cursor) = &preserved
        {
            self.restore_cursor(cursor);
        }
        if let Some(cursor) = preserved
            && cursor.owned
        {
            self.untrack_pin(cursor.tracked);
        }
        result
    }

    /// Record cursor spacing before columns or source lifetimes change.
    fn preserve_cursor(&mut self, cursor: Cursor) -> Option<PreservedCursor> {
        let pin = match cursor.pin {
            Some(handle) => *self.tracked_pin(handle)?,
            None => self.pin(crate::Point::Active(crate::Coordinate {
                x: cursor.x,
                y: u32::from(cursor.y),
            }))?,
        };
        let wrapped = self.wrapped_above(pin);
        Some(PreservedCursor {
            tracked: cursor.pin.or_else(|| self.track_pin(pin))?,
            owned: cursor.pin.is_none(),
            remaining: self.rows().saturating_sub(cursor.y.saturating_add(1)),
            wrapped,
        })
    }

    /// Count continuations only when the cursor is in the active region.
    fn wrapped_above(&self, pin: Pin) -> usize {
        let active = self.get_top_left(Tag::Active);
        if pin.before(self, active) {
            return 0;
        }
        let mut iterator = pin.row_iterator(Direction::LeftUp, Some(active));
        let mut count = 0;
        while let Some(pin) = iterator.next(self) {
            if pin
                .row_and_cell(self)
                .is_some_and(|cell| cell.0.wrap_continuation())
            {
                count += 1;
            }
        }
        count
    }

    /// Restore blank rows beneath an active cursor without pulling history down.
    fn restore_cursor(&mut self, cursor: &PreservedCursor) {
        let Some(pin) = self.tracked_pin(cursor.tracked).copied() else {
            return;
        };
        let Some(point) = self.point_from_pin(Tag::Active, pin) else {
            return;
        };
        let current = usize::from(self.rows()).saturating_sub(
            usize::try_from(point.coord().y)
                .unwrap_or(usize::MAX)
                .saturating_add(1),
        );
        let required = usize::from(cursor.remaining)
            .saturating_sub(self.wrapped_above(pin).saturating_sub(cursor.wrapped))
            .saturating_sub(current);
        for _ in 0..required {
            self.grow();
        }
    }

    /// Replace the linked output while releasing each consumed source page.
    fn reflow_columns(
        &mut self,
        cols: NonZeroU16,
        cursor: Option<TrackedPin>,
    ) -> Result<(), StorageError> {
        let first = self.first;
        let cap = Self::resized_capacity(
            self.page(first).ok_or(StorageError::OutOfMemory)?,
            cols.get(),
        );
        let mut page = Page::init(cap);
        page.size.rows = 1;
        let destination = self.nodes.insert(Node {
            page,
            prev: None,
            next: None,
        });
        self.page_size += Self::page_charge(cap);
        self.first = destination;
        self.last = destination;
        self.cols = cols;
        let mut output = ReflowCursor::new(destination);
        let mut next = Some(first);
        let mut result = Ok(());
        while let Some(key) = next {
            let Some(source) = self.nodes.remove(key) else {
                break;
            };
            next = source.next;
            self.page_size -= Self::page_charge(source.page.capacity);
            if result.is_ok() {
                result = output.reflow_page(self, key, &source.page, cursor);
            }
            // Failed copies retain emitted content and release unread source ownership.
            for pin in self.tracked.values_mut().filter(|pin| pin.node == key) {
                *pin = output.pin;
                pin.garbage = result.is_err();
            }
        }
        self.total_rows = output.total_rows;
        for _ in self.total_rows..usize::from(self.rows()) {
            self.grow();
        }
        self.resize_viewport();
        result
    }
}

impl ReflowCursor {
    /// Consume one page in source order before releasing its storage.
    fn reflow_page(
        &mut self,
        list: &mut PageList,
        key: NodeId,
        source: &Page,
        cursor: Option<TrackedPin>,
    ) -> Result<(), StorageError> {
        for y in 0..source.size.rows {
            self.reflow_row(
                list,
                source,
                Pin {
                    node: key,
                    x: 0,
                    y,
                    garbage: false,
                },
                cursor,
            )?;
        }
        Ok(())
    }

    /// Rewrap one source row, retaining meaningful blanks and prompt metadata.
    fn reflow_row(
        &mut self,
        list: &mut PageList,
        source: &Page,
        origin: Pin,
        cursor: Option<TrackedPin>,
    ) -> Result<(), StorageError> {
        let row = *source.get_row(origin.y);
        let len = self.meaningful_columns(list, source, origin, cursor);
        if len == 0 {
            if !row.wrap_continuation() {
                self.new_rows += 1;
            }
            return Ok(());
        }
        let cap = source
            .capacity
            .adjust(Adjustment {
                cols: Some(list.cols()),
            })
            .unwrap_or(crate::page::layout::Capacity {
                cols: list.cols(),
                rows: source.size.rows.min(STD_CAPACITY.rows),
                ..source.capacity
            });
        while self.new_rows > 0 {
            self.next_row(list, cap);
            self.new_rows -= 1;
        }
        if let Some(page) = list.page_mut(self.pin.node) {
            page.get_row_mut(self.pin.y)
                .set_semantic_prompt(row.semantic_prompt());
        }
        let mut x = 0;
        while x < len {
            if self.pending_wrap {
                self.wrap_row(list, cap, row.semantic_prompt());
            }
            match self.write_cell(list, source, Pin { x, ..origin })? {
                Written::One => x += 1,
                Written::Repeat => {}
                Written::Pair => {
                    x += 2;
                }
            }
        }
        if !row.wrap() {
            self.new_rows += 1;
        }
        Ok(())
    }

    /// Include prompt and retained cells while trimming nonsemantic right blanks.
    fn meaningful_columns(
        &self,
        list: &mut PageList,
        source: &Page,
        origin: Pin,
        cursor: Option<TrackedPin>,
    ) -> u16 {
        let row = *source.get_row(origin.y);
        let mut len = source.size.cols;
        if !row.wrap() {
            while len > 0 && source.get_cells(origin.y)[usize::from(len - 1)].is_empty() {
                len -= 1;
            }
            if len == 0 && row.semantic_prompt() != SemanticPrompt::None {
                len = 1;
            }
        }
        let cursor_key = cursor.map(|handle| handle.0);
        let remaining = list.cols() - 1 - self.pin.x;
        for (handle, pin) in list
            .tracked
            .iter_mut()
            .filter(|(_, p)| p.node == origin.node && p.y == origin.y)
        {
            if Some(handle) == cursor_key {
                continue;
            }
            if pin.x >= len {
                pin.x = pin.x.min(remaining);
            }
            len = len.max(pin.x + 1);
        }
        if let Some(pin) = cursor.and_then(|handle| list.tracked_pin(handle))
            && pin.node == origin.node
            && pin.y == origin.y
        {
            len = len.max(pin.x + 1);
        }
        len
    }

    /// Emit a soft break and carry the source prompt role to its continuation.
    fn wrap_row(
        &mut self,
        list: &mut PageList,
        cap: crate::page::layout::Capacity,
        prompt: SemanticPrompt,
    ) {
        if let Some(page) = list.page_mut(self.pin.node) {
            page.get_row_mut(self.pin.y).set_wrap(true);
        }
        self.next_row(list, cap);
        if let Some(page) = list.page_mut(self.pin.node) {
            let destination = page.get_row_mut(self.pin.y);
            destination.set_semantic_prompt(prompt);
            destination.set_wrap_continuation(true);
        }
    }

    /// Move retained source coordinates at the moment their cell is consumed.
    pub(super) fn remap_cell(&self, list: &mut PageList, source: NodeId, position: CellPosition) {
        for pin in list
            .tracked
            .values_mut()
            .filter(|p| p.node == source && p.x == position.x && p.y == position.y)
        {
            *pin = Pin {
                garbage: pin.garbage,
                ..self.pin
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meaningful_columns_adjusts_saved_pins_before_extending_for_cursor() {
        for cursor_first in [true, false] {
            let mut list = PageList::init(
                NonZeroU16::new(10).unwrap(),
                NonZeroU16::new(5).unwrap(),
                None,
            );
            let origin = list
                .pin(crate::Point::Active(crate::Coordinate::default()))
                .unwrap();
            for _ in 0..2 {
                list.track_pin(Pin { x: 9, ..origin }).unwrap();
            }
            // Assign cursor roles to the first and last occupied slots.
            let handles: Vec<_> = list.tracked.iter().map(|(key, _)| key).collect();
            let cursor = if cursor_first {
                handles[0]
            } else {
                *handles.last().unwrap()
            };
            for pin in list.tracked.values_mut() {
                pin.x = 9;
            }
            let source = Page::init(crate::page::layout::Capacity {
                cols: 10,
                rows: 1,
                ..crate::page::layout::Capacity::default()
            });
            list.cols = NonZeroU16::new(5).unwrap();
            let output = ReflowCursor::new(origin.node);
            assert_eq!(
                output.meaningful_columns(&mut list, &source, origin, Some(TrackedPin(cursor))),
                10
            );
            for (handle, pin) in list.tracked.iter() {
                assert_eq!(pin.x, if handle == cursor { 9 } else { 4 });
            }
        }
    }
}
