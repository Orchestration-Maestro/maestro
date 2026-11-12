//! Inclusive ranges over retained screen pins.

use crate::{Pin, Screen, point::Tag};

/// Direction between the authored endpoints of a range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Top-left to bottom-right, including a single cell.
    Forward,
    /// Bottom-right to top-left.
    Reverse,
    /// Top-right to bottom-left of a rectangle.
    MirroredForward,
    /// Bottom-left to top-right of a rectangle.
    MirroredReverse,
}

/// Inclusive endpoints, valid until their screen storage changes.
#[derive(Clone, Copy, Debug)]
pub struct Selection {
    /// Authored starting pin, not necessarily the first cell.
    pub start: Pin,
    /// Authored ending pin, not necessarily the last cell.
    pub end: Pin,
    /// Whether each included row has the same column interval.
    pub rectangle: bool,
}

impl Selection {
    /// Create an untracked range without reordering its endpoints.
    #[must_use]
    pub fn init(start: Pin, end: Pin, rectangle: bool) -> Self {
        Self {
            start,
            end,
            rectangle,
        }
    }

    /// Return the top-left corner, clamped to its page's width.
    ///
    /// Returns `None` if either endpoint has expired.
    #[must_use]
    pub fn top_left(self, screen: &Screen) -> Option<Pin> {
        let (mut pin, x) = match self.order(screen)? {
            Order::Forward => return Some(self.start),
            Order::Reverse => return Some(self.end),
            Order::MirroredForward => (self.start, self.end.x),
            Order::MirroredReverse => (self.end, self.start.x),
        };
        pin.x = x.min(screen.pages.page(pin.node)?.size.cols - 1);
        Some(pin)
    }

    /// Return the bottom-right corner, clamped to its page's width.
    ///
    /// Returns `None` if either endpoint has expired.
    #[must_use]
    pub fn bottom_right(self, screen: &Screen) -> Option<Pin> {
        let (mut pin, x) = match self.order(screen)? {
            Order::Forward => return Some(self.end),
            Order::Reverse => return Some(self.start),
            Order::MirroredForward => (self.end, self.start.x),
            Order::MirroredReverse => (self.start, self.end.x),
        };
        pin.x = x.min(screen.pages.page(pin.node)?.size.cols - 1);
        Some(pin)
    }

    /// Return an untracked range in the requested orientation.
    ///
    /// A matching orientation preserves authored endpoints. Otherwise mirrored
    /// requests use forward order. Returns `None` for expired endpoints.
    #[must_use]
    pub fn ordered(self, screen: &Screen, desired: Order) -> Option<Self> {
        if self.order(screen)? == desired {
            return Some(self);
        }
        let tl = self.top_left(screen)?;
        let br = self.bottom_right(screen)?;
        Some(if desired == Order::Reverse {
            Self::init(br, tl, self.rectangle)
        } else {
            Self::init(tl, br, self.rectangle)
        })
    }

    /// Whether a valid screen pin lies inside the inclusive range.
    ///
    /// Expired pins or endpoints are not contained.
    #[must_use]
    pub fn contains(self, screen: &Screen, pin: Pin) -> bool {
        let Some((tl, br, p)) = self.coordinates(screen, pin) else {
            return false;
        };
        if self.rectangle {
            return p.y >= tl.y && p.y <= br.y && p.x >= tl.x && p.x <= br.x;
        }
        if tl.y == br.y {
            return p.y == tl.y && p.x >= tl.x && p.x <= br.x;
        }
        if p.y == tl.y {
            return p.x >= tl.x;
        }
        if p.y == br.y {
            return p.x <= br.x;
        }
        p.y > tl.y && p.y < br.y
    }

    /// Restrict the range to a row, clamping columns to that page's width.
    ///
    /// Returns `None` outside the range or for expired pins.
    #[must_use]
    pub fn contained_row(self, screen: &Screen, pin: Pin) -> Option<Self> {
        let (tl, br, p) = self.coordinates(screen, pin)?;
        if p.y < tl.y || p.y > br.y {
            return None;
        }
        let max_x = screen.pages.page(pin.node)?.size.cols - 1;
        if self.rectangle {
            return Some(Self::init(
                Pin {
                    x: tl.x.min(max_x),
                    ..pin
                },
                Pin {
                    x: br.x.min(max_x),
                    ..pin
                },
                true,
            ));
        }
        let start = if p.y == tl.y {
            self.top_left(screen)?
        } else {
            Pin { x: 0, ..pin }
        };
        let end = if p.y == br.y {
            self.bottom_right(screen)?
        } else {
            Pin { x: max_x, ..pin }
        };
        Some(Self::init(start, end, false))
    }

    /// Project ordered endpoints and a candidate into screen coordinates.
    fn coordinates(
        self,
        screen: &Screen,
        pin: Pin,
    ) -> Option<(crate::Coordinate, crate::Coordinate, crate::Coordinate)> {
        let pages = &screen.pages;
        Some((
            pages
                .point_from_pin(Tag::Screen, self.top_left(screen)?)?
                .coord(),
            pages
                .point_from_pin(Tag::Screen, self.bottom_right(screen)?)?
                .coord(),
            pages.point_from_pin(Tag::Screen, pin)?.coord(),
        ))
    }

    /// Classify endpoint orientation, or return `None` for expired pins.
    #[must_use]
    pub fn order(self, screen: &Screen) -> Option<Order> {
        let start = screen
            .pages
            .point_from_pin(Tag::Screen, self.start)?
            .coord();
        let end = screen.pages.point_from_pin(Tag::Screen, self.end)?.coord();
        if self.rectangle {
            if (start.y > end.y && start.x >= end.x) || (start.y >= end.y && start.x > end.x) {
                return Some(Order::Reverse);
            }
            if start.y > end.y && start.x < end.x {
                return Some(Order::MirroredReverse);
            }
            if start.y < end.y && start.x > end.x {
                return Some(Order::MirroredForward);
            }
            return Some(Order::Forward);
        }
        Some(if (start.y, start.x) <= (end.y, end.x) {
            Order::Forward
        } else {
            Order::Reverse
        })
    }
}
