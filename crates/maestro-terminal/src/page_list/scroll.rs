//! Viewport position and cached scrollbar observations.

use super::{PageList, Pin};
use crate::point::Tag;

/// How the visible area follows retained content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewport {
    /// Follow the editable area as history grows.
    Active,
    /// Follow the oldest retained row.
    Top,
    /// Follow the reserved retained position.
    Pin,
}

/// A viewport movement within already retained history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scroll {
    /// Return to the editable area.
    Active,
    /// Show the oldest retained row.
    Top,
    /// Show an absolute row, clamping at the editable area.
    Row(usize),
    /// Move by a signed row distance with saturation.
    DeltaRow(isize),
}

/// Scrollable length and current visible interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scrollbar {
    /// Retained row count.
    pub total: usize,
    /// Visible top row relative to the oldest retained row.
    pub offset: usize,
    /// Visible row count.
    pub len: usize,
}

impl Scrollbar {
    /// Empty scrollable area.
    pub const ZERO: Self = Self {
        total: 0,
        offset: 0,
        len: 0,
    };

    /// Compare the complete visible interval.
    #[must_use]
    pub fn eql(self, other: Self) -> bool {
        self == other
    }
}

impl PageList {
    /// Move the viewport without allocating pages or creating history.
    pub fn scroll(&mut self, behavior: Scroll) {
        if self.explicit_max_size == Some(0) {
            self.viewport = Viewport::Active;
            return;
        }
        match behavior {
            Scroll::Active => self.viewport = Viewport::Active,
            Scroll::Top | Scroll::Row(0) => self.viewport = Viewport::Top,
            Scroll::Row(row) => self.scroll_to_row(row),
            Scroll::DeltaRow(delta) => {
                if (self.viewport == Viewport::Active && delta >= 0)
                    || (self.viewport == Viewport::Top && delta <= 0)
                {
                    return;
                }
                let row = self.viewport_row_offset().saturating_add_signed(delta);
                if row >= self.history_rows() {
                    self.viewport = Viewport::Active;
                } else if row == 0 {
                    self.viewport = Viewport::Top;
                } else {
                    self.scroll_to_row(row);
                }
            }
        }
    }

    /// Move nonempty active rows into history, leaving the new active area blank.
    pub fn scroll_clear(&mut self) {
        let mut rows = self.row_iterator(
            super::Direction::LeftUp,
            crate::Point::Active(crate::Coordinate::default()),
            None,
        );
        let mut non_empty = usize::from(self.rows());
        while let Some(pin) = rows.next(self) {
            if pin
                .cells(self, super::CellSubset::All)
                .is_some_and(|cells| cells.iter().any(|cell| !cell.is_empty()))
            {
                break;
            }
            non_empty -= 1;
        }
        for _ in 0..non_empty {
            self.grow();
        }
    }

    /// Set an absolute historical position and cache its known offset.
    fn scroll_to_row(&mut self, row: usize) {
        if row >= self.history_rows() {
            self.viewport = Viewport::Active;
            return;
        }
        let cached = self
            .viewport_pin_row_offset
            .filter(|_| self.viewport == Viewport::Pin)
            .and_then(|offset| {
                let pin = *self.tracked_pin(self.viewport_pin)?;
                if row >= offset {
                    pin.down(self, row - offset)
                } else {
                    pin.up(self, offset - row)
                }
            });
        let Some(pin) = cached.or_else(|| self.pin_at_row(row, 0)) else {
            return;
        };
        if let Some(position) = self.tracked.get_mut(self.viewport_pin.0) {
            *position = pin;
        }
        self.viewport = Viewport::Pin;
        self.viewport_pin_row_offset = Some(row);
    }

    /// Read the current scrollbar, hiding retained allocation slack in no-history mode.
    pub fn scrollbar(&mut self) -> Scrollbar {
        if self.explicit_max_size == Some(0) {
            return Scrollbar {
                total: usize::from(self.rows()),
                offset: 0,
                len: usize::from(self.rows()),
            };
        }
        Scrollbar {
            total: self.total_rows,
            offset: self.viewport_row_offset(),
            len: usize::from(self.rows()),
        }
    }

    /// Resolve a pinned viewport once after edits invalidate its absolute-row cache.
    fn viewport_row_offset(&mut self) -> usize {
        match self.viewport {
            Viewport::Top => return 0,
            Viewport::Active => return self.history_rows(),
            Viewport::Pin => {}
        }
        if let Some(offset) = self.viewport_pin_row_offset {
            return offset;
        }
        let Some(pin) = self.tracked_pin(self.viewport_pin).copied() else {
            return 0;
        };
        let mut current = Some(self.last);
        let mut below = 0;
        while let Some(key) = current {
            let Some(node) = self.nodes.get(key) else {
                break;
            };
            below += usize::from(node.page.size.rows);
            if key == pin.node {
                let offset = self.total_rows - (below - usize::from(pin.y));
                self.viewport_pin_row_offset = Some(offset);
                return offset;
            }
            current = node.prev;
        }
        0
    }

    /// Keep the viewport on a valid historical row after removing an oldest page.
    pub(super) fn fixup_viewport(&mut self, removed: usize) {
        match self.viewport {
            Viewport::Active => return,
            Viewport::Top => {
                if self.history_rows() == 0 {
                    self.viewport = Viewport::Active;
                }
                return;
            }
            Viewport::Pin => {}
        }
        let pin = self.tracked_pin(self.viewport_pin).copied();
        let active = self.get_top_left(Tag::Active);
        if pin.is_some_and(|pin| pin.eql(active) || active.before(self, pin)) {
            self.viewport = Viewport::Active;
            return;
        }
        if let Some(offset) = self.viewport_pin_row_offset {
            if offset < removed {
                self.viewport = Viewport::Top;
            } else {
                self.viewport_pin_row_offset = Some(offset - removed);
            }
        }
    }

    /// Resolve the current viewport origin without retaining an extra page borrow.
    pub(super) fn viewport_origin(&self) -> Pin {
        match self.viewport {
            Viewport::Active => self.get_top_left(Tag::Active),
            Viewport::Top => self.get_top_left(Tag::Screen),
            Viewport::Pin => self
                .tracked_pin(self.viewport_pin)
                .copied()
                .unwrap_or_else(|| self.get_top_left(Tag::Active)),
        }
    }
}
