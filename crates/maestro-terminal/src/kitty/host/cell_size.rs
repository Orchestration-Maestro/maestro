//! Host terminal cell size in pixels.

use super::Rect;

/// Window size the terminal backend reports, in cells and pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowSize {
    /// Terminal columns.
    pub columns: u16,
    /// Terminal rows.
    pub rows: u16,
    /// Terminal width in pixels.
    pub width: u16,
    /// Terminal height in pixels.
    pub height: u16,
}

/// Host terminal cell size in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostCellSize {
    /// Cell width in pixels.
    pub width_px: u32,
    /// Cell height in pixels.
    pub height_px: u32,
}

impl HostCellSize {
    /// Cell size of a reported window for one pane area, or none when the
    /// report carries no usable cell geometry.
    #[must_use]
    pub fn from_window(window: WindowSize, area: Rect) -> Option<Self> {
        if window.columns == 0 || window.rows == 0 || window.width == 0 || window.height == 0 {
            return None;
        }
        Some(
            Self {
                width_px: (u32::from(window.width) / u32::from(window.columns)).max(1),
                height_px: (u32::from(window.height) / u32::from(window.rows)).max(1),
            }
            .for_area(area),
        )
    }

    /// Whether both cell dimensions are known.
    #[must_use]
    pub fn is_known(self) -> bool {
        self.width_px > 0 && self.height_px > 0
    }

    /// Assumed cell size, usable only for a non-empty area.
    #[must_use]
    pub fn fallback_for_area(area: Rect) -> Self {
        Self {
            width_px: 8,
            height_px: 16,
        }
        .for_area(area)
    }

    /// Unknown geometry for an area with no cells.
    fn for_area(self, area: Rect) -> Self {
        if area.width == 0 || area.height == 0 {
            return Self::default();
        }
        self
    }
}
