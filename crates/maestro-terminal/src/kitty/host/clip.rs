//! Clipping of a placement to the visible pane area.

use super::{ClippedPlacement, HostPlacement, Rect, identity::kitty_format_code};
use crate::kitty::graphics::render::PlacementRenderInfo;

/// Placement geometry after clipping to the visible pane area.
#[derive(Clone, Copy)]
struct Destination {
    /// First shown column.
    x: u16,
    /// First shown row.
    y: u16,
    /// Shown columns.
    cols: u32,
    /// Shown rows.
    rows: u32,
    /// Columns cropped from the leading edge.
    left_clip: u32,
    /// Rows cropped from the top edge.
    top_clip: u32,
}

/// Clipped geometry of a placement, or none when no cell of it is visible.
///
/// The returned format code is the Kitty code of the placement's pixel
/// encoding, so callers derive it once instead of passing it around.
#[must_use]
pub fn clipped_placement(placement: &HostPlacement) -> Option<(ClippedPlacement, u32)> {
    let destination = destination(placement.area, placement.placement.render)?;
    let (source_x, source_y, source_width, source_height) = source_rect(placement, destination)?;
    Some((
        ClippedPlacement {
            x: destination.x,
            y: destination.y,
            cols: destination.cols,
            rows: destination.rows,
            source_x,
            source_y,
            source_width,
            source_height,
            x_offset: offset(placement.placement.x_offset, destination.left_clip),
            y_offset: offset(placement.placement.y_offset, destination.top_clip),
        },
        kitty_format_code(placement.placement.format),
    ))
}

/// Visible destination cells of a placement inside a pane area.
fn destination(area: Rect, render: PlacementRenderInfo) -> Option<Destination> {
    if area.width == 0 || area.height == 0 || render.grid_cols == 0 || render.grid_rows == 0 {
        return None;
    }
    let left_clip = cropped(render.viewport_col);
    let top_clip = cropped(render.viewport_row);
    let column = widen(render.viewport_col.max(0));
    let row = widen(render.viewport_row.max(0));
    if column >= u32::from(area.width) || row >= u32::from(area.height) {
        return None;
    }
    let cols = render
        .grid_cols
        .saturating_sub(left_clip)
        .min(u32::from(area.width) - column);
    let rows = render
        .grid_rows
        .saturating_sub(top_clip)
        .min(u32::from(area.height) - row);
    if cols == 0 || rows == 0 {
        return None;
    }
    Some(Destination {
        x: area
            .x
            .saturating_add(u16::try_from(column).unwrap_or(u16::MAX)),
        y: area
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
        cols,
        rows,
        left_clip,
        top_clip,
    })
}

/// Source rectangle in image pixels for the visible cells.
fn source_rect(
    placement: &HostPlacement,
    destination: Destination,
) -> Option<(u32, u32, u32, u32)> {
    let render = placement.placement.render;
    let cell_width = placement.cell_size.width_px;
    let cell_height = placement.cell_size.height_px;
    let image_width = placement.placement.image_width;
    let image_height = placement.placement.image_height;
    let source_width = full(render.source_width, image_width);
    let source_height = full(render.source_height, image_height);
    let pixel_width = render
        .pixel_width
        .max(render.grid_cols.saturating_mul(cell_width))
        .max(1);
    let pixel_height = render
        .pixel_height
        .max(render.grid_rows.saturating_mul(cell_height))
        .max(1);
    let source_x = render.source_x.saturating_add(scale_pixels(
        destination.left_clip.saturating_mul(cell_width),
        source_width,
        pixel_width,
    ));
    let source_y = render.source_y.saturating_add(scale_pixels(
        destination.top_clip.saturating_mul(cell_height),
        source_height,
        pixel_height,
    ));
    let width = shown(
        destination.cols.saturating_mul(cell_width),
        source_width,
        pixel_width,
        image_width.saturating_sub(source_x),
    );
    let height = shown(
        destination.rows.saturating_mul(cell_height),
        source_height,
        pixel_height,
        image_height.saturating_sub(source_y),
    );
    if width == 0 || height == 0 {
        return None;
    }
    Some((source_x, source_y, width, height))
}

/// In-cell offset kept only on an uncropped side.
fn offset(offset: u32, cropped: u32) -> u32 {
    if cropped == 0 { offset } else { 0 }
}

/// Shown source extent, at least one pixel and never past the image edge.
fn shown(value: u32, source: u32, dest: u32, remaining: u32) -> u32 {
    scale_pixels(value, source, dest).max(1).min(remaining)
}

/// Image dimension when the renderer reports none.
fn full(reported: u32, image: u32) -> u32 {
    if reported == 0 { image } else { reported }
}

/// Scale a pixel count from the destination grid onto the source image.
fn scale_pixels(value: u32, source: u32, dest: u32) -> u32 {
    let scaled = u64::from(value).saturating_mul(u64::from(source)) / u64::from(dest.max(1));
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

/// Cells cut off the leading edge by a negative viewport coordinate.
fn cropped(value: i64) -> u32 {
    widen(value.min(0).saturating_neg())
}

/// Widen a non-negative viewport coordinate, which a real pane keeps inside the unsigned range.
fn widen(value: i64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
