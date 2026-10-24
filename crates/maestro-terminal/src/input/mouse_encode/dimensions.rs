//! Pixel geometry used to translate mouse positions into terminal coordinates.

use num_traits::ToPrimitive;

/// Pixel dimensions and padding, already scaled for the surface's DPI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Full surface dimensions, including padding.
    pub screen: ScreenSize,
    /// Nonzero dimensions of one rendered cell.
    pub cell: CellSize,
    /// Explicit space around the terminal grid.
    pub padding: Padding,
}

impl Size {
    /// Calculate the visible grid, retaining at least one row and column.
    #[must_use]
    pub fn grid(self) -> GridSize {
        GridSize::new(self.terminal(), self.cell)
    }

    /// Return surface dimensions with padding removed.
    #[must_use]
    pub fn terminal(self) -> ScreenSize {
        self.screen.sub_padding(self.padding)
    }
}

/// Pixel dimensions of one rendered cell; both dimensions must be nonzero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSize {
    /// Cell width.
    pub width: u32,
    /// Cell height.
    pub height: u32,
}

/// Pixel dimensions of a surface or unpadded terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl ScreenSize {
    /// Remove padding, saturating at zero for an undersized surface.
    #[must_use]
    pub fn sub_padding(self, padding: Padding) -> Self {
        Self {
            width: self
                .width
                .saturating_sub(padding.left.saturating_add(padding.right)),
            height: self
                .height
                .saturating_sub(padding.top.saturating_add(padding.bottom)),
        }
    }

    /// Return leftover pixels on the right and bottom of the rendered grid.
    #[must_use]
    pub fn blank_padding(self, padding: Padding, grid: GridSize, cell: CellSize) -> Padding {
        let terminal = self.sub_padding(padding);
        Padding {
            right: terminal
                .width
                .saturating_sub(u32::from(grid.columns) * cell.width),
            bottom: terminal
                .height
                .saturating_sub(u32::from(grid.rows) * cell.height),
            ..Padding::default()
        }
    }
}

/// Visible grid dimensions in cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridSize {
    /// Number of columns.
    pub columns: u16,
    /// Number of rows.
    pub rows: u16,
}

impl GridSize {
    /// Calculate the grid from unpadded pixel dimensions.
    #[must_use]
    pub fn new(screen: ScreenSize, cell: CellSize) -> Self {
        let mut grid = Self::default();
        grid.update(screen, cell);
        grid
    }

    /// Recalculate the grid, truncating partial cells and retaining one cell.
    pub fn update(&mut self, screen: ScreenSize, cell: CellSize) {
        self.columns = cell_count(screen.width, cell.width);
        self.rows = cell_count(screen.height, cell.height);
    }
}

/// Calculate one grid dimension with single-precision renderer arithmetic.
fn cell_count(pixels: u32, cell: u32) -> u16 {
    let pixels = pixels.to_f32().unwrap_or_default();
    let cell = cell.to_f32().unwrap_or(1.0);
    (pixels / cell)
        .clamp(1.0, f32::from(u16::MAX))
        .to_u16()
        .unwrap_or(1)
}

/// Explicit surface padding in scaled pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Padding {
    /// Space above the grid.
    pub top: u32,
    /// Space below the grid.
    pub bottom: u32,
    /// Space left of the grid.
    pub left: u32,
    /// Space right of the grid.
    pub right: u32,
}

/// Coordinate space to convert into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// Surface pixels, including padding.
    Surface,
    /// Terminal pixels, excluding padding.
    Terminal,
    /// Zero-based visible grid cell.
    Grid,
}

/// A coordinate valid within a stable size observation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinate {
    /// Surface pixels, with negative and off-surface values permitted.
    Surface {
        /// Horizontal position.
        x: f64,
        /// Vertical position.
        y: f64,
    },
    /// Unpadded terminal pixels, not clamped to the grid.
    Terminal {
        /// Horizontal position.
        x: f64,
        /// Vertical position.
        y: f64,
    },
    /// Zero-based cell coordinates.
    Grid {
        /// Column.
        x: u16,
        /// Row.
        y: u16,
    },
}

impl Coordinate {
    /// Convert spaces, clamping only when converting into grid coordinates.
    #[must_use]
    pub fn convert(self, to: Space, size: Size) -> Self {
        if matches!(
            (self, to),
            (Self::Surface { .. }, Space::Surface)
                | (Self::Terminal { .. }, Space::Terminal)
                | (Self::Grid { .. }, Space::Grid)
        ) {
            return self;
        }
        let (x, y) = self.surface(size);
        let tx = x - f64::from(size.padding.left);
        let ty = y - f64::from(size.padding.top);
        match to {
            Space::Surface => Self::Surface { x, y },
            Space::Terminal => Self::Terminal { x: tx, y: ty },
            Space::Grid => {
                let grid = size.grid();
                Self::Grid {
                    x: (tx / f64::from(size.cell.width))
                        .clamp(0.0, f64::from(grid.columns - 1))
                        .to_u16()
                        .unwrap_or_default(),
                    y: (ty / f64::from(size.cell.height))
                        .clamp(0.0, f64::from(grid.rows - 1))
                        .to_u16()
                        .unwrap_or_default(),
                }
            }
        }
    }

    /// Convert any coordinate to surface pixels.
    fn surface(self, size: Size) -> (f64, f64) {
        let left = f64::from(size.padding.left);
        let top = f64::from(size.padding.top);
        match self {
            Self::Surface { x, y } => (x, y),
            Self::Terminal { x, y } => (x + left, y + top),
            Self::Grid { x, y } => (
                f64::from(x) * f64::from(size.cell.width) + left,
                f64::from(y) * f64::from(size.cell.height) + top,
            ),
        }
    }
}
