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
    rounded(f64::from(size) * (f64::from(numerator) / f64::from(denominator)))
}

/// Ceiling division with empty geometry for unknown cell dimensions.
fn ceil(value: u32, divisor: u32) -> u32 {
    if divisor == 0 {
        0
    } else {
        value.div_ceil(divisor)
    }
}

/// Combined physical placement dimensions, visibility and clamped source pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlacementRenderInfo {
    /// Destination width in pixels.
    pub pixel_width: u32,
    /// Destination height in pixels.
    pub pixel_height: u32,
    /// Occupied cell columns.
    pub grid_cols: u32,
    /// Occupied cell rows.
    pub grid_rows: u32,
    /// Viewport-relative origin column.
    pub viewport_col: i64,
    /// Viewport-relative origin row, negative above the viewport.
    pub viewport_row: i64,
    /// Whether any occupied row intersects the viewport.
    pub viewport_visible: bool,
    /// Clamped source left pixel.
    pub source_x: u32,
    /// Clamped source top pixel.
    pub source_y: u32,
    /// Clamped source width.
    pub source_width: u32,
    /// Clamped source height.
    pub source_height: u32,
}

impl Placement {
    /// Clamp the source rectangle, interpreting zero sizes as full dimensions.
    #[must_use]
    pub fn source_rect(&self, image: &Image) -> (u32, u32, u32, u32) {
        let x = self.source_x.min(image.width);
        let y = self.source_y.min(image.height);
        let width = if self.source_width == 0 {
            image.width
        } else {
            self.source_width
        };
        let height = if self.source_height == 0 {
            image.height
        } else {
            self.source_height
        };
        (
            x,
            y,
            width.min(image.width - x),
            height.min(image.height - y),
        )
    }

    /// Viewport-relative origin and row visibility, including partly hidden images.
    #[must_use]
    pub fn viewport_pos(
        &self,
        image: &Image,
        pages: &PageList,
        cell: (u32, u32),
    ) -> (i64, i64, bool) {
        let Location::Pin(handle) = self.location else {
            return (0, 0, false);
        };
        let Some(pin) = pages
            .tracked_pin(handle)
            .and_then(|pin| pages.point_from_pin(crate::point::Tag::Screen, *pin))
        else {
            return (0, 0, false);
        };
        let origin = pages.get_top_left(crate::point::Tag::Viewport);
        let Some(origin) = pages.point_from_pin(crate::point::Tag::Screen, origin) else {
            return (0, 0, false);
        };
        let row = i64::from(pin.coord().y) - i64::from(origin.coord().y);
        let rows = self.grid_size(image, cell).1;
        (
            i64::from(pin.coord().x),
            row,
            row + i64::from(rows) > 0 && row < i64::from(pages.rows()),
        )
    }

    /// Read every renderer-facing physical placement property in one operation.
    #[must_use]
    pub fn render_info(
        &self,
        image: &Image,
        pages: &PageList,
        cell: (u32, u32),
    ) -> PlacementRenderInfo {
        let (pixel_width, pixel_height) = self.pixel_size(image, cell);
        let (grid_cols, grid_rows) = self.grid_size(image, cell);
        let (viewport_col, viewport_row, viewport_visible) = self.viewport_pos(image, pages, cell);
        let (source_x, source_y, source_width, source_height) = self.source_rect(image);
        PlacementRenderInfo {
            pixel_width,
            pixel_height,
            grid_cols,
            grid_rows,
            viewport_col,
            viewport_row,
            viewport_visible,
            source_x,
            source_y,
            source_width,
            source_height,
        }
    }
}

/// Pixel rectangles positioning a placeholder fragment from a retained origin.
#[derive(Clone, Copy, Debug)]
pub struct RenderPlacement {
    /// Horizontal destination offset within the first cell.
    pub offset_x: u32,
    /// Vertical destination offset within the first cell.
    pub offset_y: u32,
    /// Source left pixel.
    pub source_x: u32,
    /// Source top pixel.
    pub source_y: u32,
    /// Source pixel width.
    pub source_width: u32,
    /// Source pixel height.
    pub source_height: u32,
    /// Destination pixel width.
    pub dest_width: u32,
    /// Destination pixel height.
    pub dest_height: u32,
    /// Visible placeholder columns.
    pub grid_cols: u32,
    /// Visible placeholder rows.
    pub grid_rows: u32,
}

/// Round a nonnegative pixel dimension without runtime overflow traps.
pub(super) fn rounded(value: f64) -> u32 {
    value
        .round()
        .to_u32()
        .unwrap_or(if value > 0.0 { u32::MAX } else { 0 })
}
