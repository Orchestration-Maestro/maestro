//! Bounded cell rectangles and split rounding.
use super::SplitDirection;
use num_traits::ToPrimitive;
/// A cell rectangle; `new` bounds extents to the coordinate domain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    /// Left coordinate.
    pub x: u16,
    /// Top coordinate.
    pub y: u16,
    /// Horizontal extent.
    pub width: u16,
    /// Vertical extent.
    pub height: u16,
}
impl Rect {
    /// Construct a rectangle whose far edges fit in u16.
    #[must_use]
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width: width.min(u16::MAX - x),
            height: height.min(u16::MAX - y),
        }
    }
    /// Saturating right edge, also for directly constructed records.
    #[must_use]
    pub fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }
    /// Saturating bottom edge, also for directly constructed records.
    #[must_use]
    pub fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }
}
/// Border flags for a pane's four edges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Borders(u8);
impl Borders {
    /// No borders.
    pub const NONE: Self = Self(0);
    /// Top edge.
    pub const TOP: Self = Self(1);
    /// Right edge.
    pub const RIGHT: Self = Self(2);
    /// Bottom edge.
    pub const BOTTOM: Self = Self(4);
    /// Left edge.
    pub const LEFT: Self = Self(8);
    /// Every edge.
    pub const ALL: Self = Self(15);

    /// Whether every edge of `other` is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether no edge is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Edges set in either flag set.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Edges of `self` that `other` does not set.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}
/// Whether two spans of cells share at least one cell, saturating at the coordinate limit.
#[must_use]
pub fn ranges_overlap(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> bool {
    a_start < b_start.saturating_add(b_len) && a_start.saturating_add(a_len) > b_start
}
/// Split cells using multiply-then-round for the first extent.
pub(super) fn split_rect(area: Rect, direction: &SplitDirection, ratio: f32) -> (Rect, Rect) {
    match direction {
        SplitDirection::Right => {
            let first_w = (f32::from(area.width) * ratio)
                .round()
                .clamp(0.0, f32::from(u16::MAX))
                .to_u16()
                .unwrap_or(0);
            let second_w = area.width.saturating_sub(first_w);
            (
                Rect::new(area.x, area.y, first_w, area.height),
                Rect::new(
                    area.x.saturating_add(first_w),
                    area.y,
                    second_w,
                    area.height,
                ),
            )
        }
        SplitDirection::Down => {
            let first_h = (f32::from(area.height) * ratio)
                .round()
                .clamp(0.0, f32::from(u16::MAX))
                .to_u16()
                .unwrap_or(0);
            let second_h = area.height.saturating_sub(first_h);
            (
                Rect::new(area.x, area.y, area.width, first_h),
                Rect::new(area.x, area.y.saturating_add(first_h), area.width, second_h),
            )
        }
    }
}
