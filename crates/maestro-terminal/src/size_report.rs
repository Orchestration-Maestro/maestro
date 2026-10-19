//! Output formats and geometry for terminal size reports.

use std::io::{self, Write};

/// Size report wire format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Unsolicited report while mode 2048 is enabled.
    Mode2048,
    /// Text area size in pixels.
    Csi14T,
    /// Cell size in pixels.
    Csi16T,
    /// Text area size in cells.
    Csi18T,
}

/// Caller-supplied geometry for a size report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Terminal row count in cells.
    pub rows: u16,
    /// Terminal column count in cells.
    pub columns: u16,
    /// Width of one cell in pixels.
    pub cell_width: u32,
    /// Height of one cell in pixels.
    pub cell_height: u32,
}

/// Encode a size report without narrowing pixel products.
///
/// # Errors
/// Returns an error if the writer cannot accept the report.
pub fn encode(writer: &mut impl Write, style: Style, size: Size) -> io::Result<()> {
    let height = u64::from(size.rows) * u64::from(size.cell_height);
    let width = u64::from(size.columns) * u64::from(size.cell_width);
    match style {
        Style::Mode2048 => write!(
            writer,
            "\x1b[48;{};{};{height};{width}t",
            size.rows, size.columns
        ),
        Style::Csi14T => write!(writer, "\x1b[4;{height};{width}t"),
        Style::Csi16T => write!(writer, "\x1b[6;{};{}t", size.cell_height, size.cell_width),
        Style::Csi18T => write!(writer, "\x1b[8;{};{}t", size.rows, size.columns),
    }
}
