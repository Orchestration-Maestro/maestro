//! Position ordering and movement through linked pages.

use super::{PageList, Pin};

impl Pin {
    /// Whether this current position precedes another in linked screen order.
    #[must_use]
    pub fn before(self, list: &PageList, other: Self) -> bool {
        if !list.pin_is_valid(self) || !list.pin_is_valid(other) {
            return false;
        }
        if self.node == other.node {
            return (self.y, self.x) < (other.y, other.x);
        }
        let mut current = list.nodes.get(self.node).and_then(|node| node.next);
        while let Some(key) = current {
            if key == other.node {
                return true;
            }
            current = list.nodes.get(key).and_then(|node| node.next);
        }
        false
    }

    /// Whether this position lies in the inclusive ordered interval.
    #[must_use]
    pub fn is_between(self, list: &PageList, top: Self, bottom: Self) -> bool {
        list.pin_is_valid(self)
            && list.pin_is_valid(top)
            && list.pin_is_valid(bottom)
            && (self.eql(top) || top.before(list, self))
            && (self.eql(bottom) || self.before(list, bottom))
    }
}

/// Result of a vertical move that can reach a screen boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offset {
    /// Full movement completed.
    Offset(Pin),
    /// Screen boundary reached before consuming the full distance.
    Overflow {
        /// Reached endpoint, retaining the admitted column.
        end: Pin,
        /// Distance beyond the endpoint.
        remaining: usize,
    },
}

impl Pin {
    /// Move left within the row; callers supply an in-row distance.
    #[must_use]
    pub fn left(self, n: usize) -> Self {
        Self {
            x: self.x.saturating_sub(u16::try_from(n).unwrap_or(u16::MAX)),
            ..self
        }
    }

    /// Move right when the requested cell remains within this row.
    #[must_use]
    pub fn right(self, list: &PageList, n: usize) -> Option<Self> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let x = usize::from(self.x).checked_add(n)?;
        (x < usize::from(list.page(self.node)?.size.cols)).then_some(Self {
            x: u16::try_from(x).ok()?,
            ..self
        })
    }

    /// Move left with saturation at the row origin.
    #[must_use]
    pub fn left_clamp(self, n: u16) -> Self {
        Self {
            x: self.x.saturating_sub(n),
            ..self
        }
    }

    /// Move right with saturation at this row's final cell.
    #[must_use]
    pub fn right_clamp(self, list: &PageList, n: u16) -> Option<Self> {
        if !list.pin_is_valid(self) {
            return None;
        }
        Some(Self {
            x: self
                .x
                .saturating_add(n)
                .min(list.page(self.node)?.size.cols - 1),
            ..self
        })
    }

    /// Move left across row boundaries, respecting each destination row's width.
    #[must_use]
    pub fn left_wrap(self, list: &PageList, mut n: usize) -> Option<Self> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let mut pin = self;
        while n > usize::from(pin.x) {
            n -= usize::from(pin.x) + 1;
            pin = pin.up(list, 1)?;
            pin.x = list.page(pin.node)?.size.cols - 1;
        }
        pin.x -= u16::try_from(n).ok()?;
        Some(pin)
    }

    /// Move right across row boundaries, respecting each destination row's width.
    #[must_use]
    pub fn right_wrap(self, list: &PageList, mut n: usize) -> Option<Self> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let mut pin = self;
        loop {
            let available = list.page(pin.node)?.size.cols - pin.x - 1;
            if n <= usize::from(available) {
                pin.x += u16::try_from(n).ok()?;
                return Some(pin);
            }
            n -= usize::from(available) + 1;
            pin = pin.down(list, 1)?;
            pin.x = 0;
        }
    }

    /// Move up, returning no position beyond the retained screen.
    #[must_use]
    pub fn up(self, list: &PageList, n: usize) -> Option<Self> {
        match self.up_overflow(list, n)? {
            Offset::Offset(pin) => Some(pin),
            Offset::Overflow { .. } => None,
        }
    }

    /// Move down, returning no position beyond the retained screen.
    #[must_use]
    pub fn down(self, list: &PageList, n: usize) -> Option<Self> {
        match self.down_overflow(list, n)? {
            Offset::Offset(pin) => Some(pin),
            Offset::Overflow { .. } => None,
        }
    }

    /// Move up or return the top endpoint with the unconsumed distance.
    #[must_use]
    pub fn up_overflow(self, list: &PageList, n: usize) -> Option<Offset> {
        self.move_rows(list, n, super::Direction::LeftUp)
    }

    /// Move down or return the bottom endpoint with the unconsumed distance.
    #[must_use]
    pub fn down_overflow(self, list: &PageList, n: usize) -> Option<Offset> {
        self.move_rows(list, n, super::Direction::RightDown)
    }

    /// Traverse each crossed page once while retaining the original column.
    fn move_rows(
        self,
        list: &PageList,
        mut n: usize,
        direction: super::Direction,
    ) -> Option<Offset> {
        if !list.pin_is_valid(self) {
            return None;
        }
        let mut pin = Self {
            garbage: false,
            ..self
        };
        loop {
            let node = list.nodes.get(pin.node)?;
            let available = match direction {
                super::Direction::LeftUp => pin.y,
                super::Direction::RightDown => node.page.size.rows - pin.y - 1,
            };
            if n <= usize::from(available) {
                let distance = u16::try_from(n).ok()?;
                pin.y = match direction {
                    super::Direction::LeftUp => pin.y - distance,
                    super::Direction::RightDown => pin.y + distance,
                };
                pin.x = self.x.min(node.page.size.cols - 1);
                return Some(Offset::Offset(pin));
            }
            n -= usize::from(available);
            let next = match direction {
                super::Direction::LeftUp => node.prev,
                super::Direction::RightDown => node.next,
            };
            let Some(next) = next else {
                pin.y = match direction {
                    super::Direction::LeftUp => 0,
                    super::Direction::RightDown => node.page.size.rows - 1,
                };
                pin.x = self.x.min(node.page.size.cols - 1);
                return Some(Offset::Overflow {
                    end: pin,
                    remaining: n,
                });
            };
            n -= 1;
            pin.node = next;
            pin.y = match direction {
                super::Direction::LeftUp => list.page(next)?.size.rows - 1,
                super::Direction::RightDown => 0,
            };
        }
    }
}
