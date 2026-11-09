//! Supplied mouse geometry and SGR report operations.

#[cfg(test)]
mod tests;

/// Parse exactly three ASCII decimal fields in a complete SGR mouse report.
#[must_use]
pub fn parse_report(data: &[u8]) -> Option<(u32, u32)> {
    let fields = maestro_tui::input_buffer::scan_sgr_mouse_fields(data)?;
    parse_number(fields.cb)?;
    let x = parse_number(fields.column)?;
    let y = parse_number(fields.row)?;
    fields.extra.is_none().then_some((x, y))
}

/// Decode a nonempty unsigned ASCII decimal value.
fn parse_number(value: &[u8]) -> Option<u32> {
    if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(value).ok()?.parse().ok()
}

/// Mouse location in pane cells or pane pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// Zero-based cell coordinates.
    Cell {
        /// Column.
        column: u16,
        /// Row.
        row: u16,
    },
    /// One-based pixel coordinates.
    Pixels {
        /// Horizontal pixel.
        x: u32,
        /// Vertical pixel.
        y: u32,
    },
}

/// Supplied host grid and pixel extents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostGeometry {
    /// Grid columns.
    pub cols: u16,
    /// Grid rows.
    pub rows: u16,
    /// Pixel width.
    pub width_px: u32,
    /// Pixel height.
    pub height_px: u32,
}

impl HostGeometry {
    /// Construct geometry only when all four dimensions are nonzero.
    #[must_use]
    pub fn new(cols: u16, rows: u16, width_px: u32, height_px: u32) -> Option<Self> {
        (cols > 0 && rows > 0 && width_px > 0 && height_px > 0).then_some(Self {
            cols,
            rows,
            width_px,
            height_px,
        })
    }

    /// Map one-based host pixels to zero-based grid cells.
    #[must_use]
    pub fn cell(self, x: u32, y: u32) -> Option<(u16, u16)> {
        Some((
            grid_cell(x.checked_sub(1)?, self.cols, self.width_px)?,
            grid_cell(y.checked_sub(1)?, self.rows, self.height_px)?,
        ))
    }

    /// Pixel boundary of a host grid column.
    fn column_boundary(self, column: u16) -> Option<u32> {
        boundary(column, self.cols, self.width_px)
    }

    /// Pixel boundary of a host grid row.
    fn row_boundary(self, row: u16) -> Option<u32> {
        boundary(row, self.rows, self.height_px)
    }
}

/// One-based host pixel location with its observed geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostPixels {
    /// Horizontal host pixel.
    pub x: u32,
    /// Vertical host pixel.
    pub y: u32,
    /// Supplied host geometry.
    pub geometry: HostGeometry,
}

impl HostPixels {
    /// Map a host pixel inside the supplied rectangle to one-based pane pixels.
    #[must_use]
    pub fn pane_position(
        self,
        inner: maestro_workspace::tiling::Rect,
        child_width_px: u32,
        child_height_px: u32,
    ) -> Option<Position> {
        let start_x = self.geometry.column_boundary(inner.x)?;
        let start_y = self.geometry.row_boundary(inner.y)?;
        let end_x = self
            .geometry
            .column_boundary(inner.x.checked_add(inner.width)?)?;
        let end_y = self
            .geometry
            .row_boundary(inner.y.checked_add(inner.height)?)?;
        let x = self.x.checked_sub(1)?.checked_sub(start_x)?;
        let y = self.y.checked_sub(1)?.checked_sub(start_y)?;
        let width = end_x.checked_sub(start_x)?;
        let height = end_y.checked_sub(start_y)?;
        if x >= width || y >= height || child_width_px == 0 || child_height_px == 0 {
            return None;
        }
        Some(Position::Pixels {
            x: scale(x, width, child_width_px)?.checked_add(1)?,
            y: scale(y, height, child_height_px)?.checked_add(1)?,
        })
    }
}

/// Compute a grid boundary without rounding the cell size first.
fn boundary(index: u16, count: u16, extent: u32) -> Option<u32> {
    if count == 0 || index > count || extent == 0 {
        return None;
    }
    u32::try_from(u64::from(index) * u64::from(extent) / u64::from(count)).ok()
}

/// Scale an admitted pixel offset with multiplication before division.
fn scale(pixel: u32, source: u32, target: u32) -> Option<u32> {
    u32::try_from(
        (u64::from(pixel) * u64::from(target) / u64::from(source))
            .min(u64::from(target.saturating_sub(1))),
    )
    .ok()
}

/// Locate an admitted pixel in the fractional host grid.
fn grid_cell(pixel: u32, count: u16, extent: u32) -> Option<u16> {
    if count == 0 || extent == 0 || pixel >= extent {
        return None;
    }
    let cell = ((u64::from(pixel) + 1) * u64::from(count) - 1) / u64::from(extent);
    u16::try_from(cell).ok().filter(|cell| *cell < count)
}

/// Replace cell coordinates while retaining button text and the terminator.
#[must_use]
pub fn report_at_cell(data: &[u8], column: u16, row: u16) -> Option<Vec<u8>> {
    let body = data.strip_prefix(b"\x1b[<")?;
    let (body, suffix) = if let Some(body) = body.strip_suffix(b"M") {
        (body, 'M')
    } else {
        (body.strip_suffix(b"m")?, 'm')
    };
    let buttons = std::str::from_utf8(body.split(|byte| *byte == b';').next()?).ok()?;
    Some(
        format!(
            "\x1b[<{buttons};{};{}{suffix}",
            u32::from(column) + 1,
            u32::from(row) + 1
        )
        .into_bytes(),
    )
}
