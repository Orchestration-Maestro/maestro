//! Placement sizing and retained screen rectangles.

use num_traits::ToPrimitive;

use super::{
    image::Image,
    storage::{Location, Placement},
};
use crate::{
    PageList,
    page_list::{Offset, Pin},
};

/// Inclusive retained rectangle occupied by a physical placement.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    /// First occupied cell.
    pub top_left: Pin,
    /// Last occupied cell, clipped to the retained screen.
    pub bottom_right: Pin,
}

impl Placement {
    /// Destination pixel width and height at the supplied cell pixel size.
    #[must_use]
    pub fn pixel_size(&self, image: &Image, cell: (u32, u32)) -> (u32, u32) {
        let width = if self.source_width > 0 {
            self.source_width
        } else {
            image.width
        };
        let height = if self.source_height > 0 {
            self.source_height
        } else {
            image.height
        };
        match (self.columns, self.rows) {
            (0, 0) => (width, height),
            (cols, 0) => {
                let w = cell.0.saturating_mul(cols);
                (w, scaled(w, height, width))
            }
            (0, rows) => {
                let h = cell.1.saturating_mul(rows);
                (scaled(h, width, height), h)
            }
            (cols, rows) => (cell.0.saturating_mul(cols), cell.1.saturating_mul(rows)),
        }
    }

    /// Occupied columns and rows, including the in-cell pixel offsets.
    #[must_use]
    pub fn grid_size(&self, image: &Image, cell: (u32, u32)) -> (u32, u32) {
        if self.columns > 0 && self.rows > 0 {
            return (self.columns, self.rows);
        }
        let (width, height) = self.pixel_size(image, cell);
        (
            ceil(width.saturating_add(self.x_offset), cell.0),
            ceil(height.saturating_add(self.y_offset), cell.1),
        )
    }

    /// Retained rectangle, or none for virtual, empty or expired placements.
    #[must_use]
    pub fn rect(&self, image: &Image, pages: &PageList, cell: (u32, u32)) -> Option<Rect> {
        let Location::Pin(handle) = self.location else {
            return None;
        };
        let top_left = *pages.tracked_pin(handle)?;
        let (cols, rows) = self.grid_size(image, cell);
        let distance = usize::try_from(rows.checked_sub(1)?).ok()?;
        let mut bottom_right = match top_left.down_overflow(pages, distance)? {
            Offset::Offset(pin) | Offset::Overflow { end: pin, .. } => pin,
        };
        let x = u32::from(top_left.x).saturating_add(cols.checked_sub(1)?);
        bottom_right.x = u16::try_from(x.min(u32::from(pages.cols() - 1))).ok()?;
        Some(Rect {
            top_left,
            bottom_right,
        })
    }
}

/// Round an aspect-preserving dimension, saturating outside the pixel range.
fn scaled(size: u32, numerator: u32, denominator: u32) -> u32 {
    let value = (f64::from(size) * (f64::from(numerator) / f64::from(denominator))).round();
    value
        .to_u32()
        .unwrap_or(if value > 0.0 { u32::MAX } else { 0 })
}

/// Ceiling division with empty geometry for unknown cell dimensions.
fn ceil(value: u32, divisor: u32) -> u32 {
    if divisor == 0 {
        0
    } else {
        value.div_ceil(divisor)
    }
}
