//! Conversions between frame buffers and the protocol's frame records.

use maestro_protocol::frames::{CellData, CursorState, FrameData};
use ratatui::buffer::{Buffer, Cell, CellDiffOption};
use ratatui::style::Color;
use std::collections::HashMap;

/// Pack a colour for a frame record.
///
/// Named colours become `0x00_00_00_XX` with `XX` from 0 (reset) to 16
/// (white), indexed colours `0x01_00_00_XX` and RGB colours `0x02_RR_GG_BB`.
#[must_use]
pub fn color_to_u32(color: Color) -> u32 {
    match color {
        Color::Reset => 0x00,
        Color::Black => 0x01,
        Color::Red => 0x02,
        Color::Green => 0x03,
        Color::Yellow => 0x04,
        Color::Blue => 0x05,
        Color::Magenta => 0x06,
        Color::Cyan => 0x07,
        Color::Gray => 0x08,
        Color::DarkGray => 0x09,
        Color::LightRed => 0x0A,
        Color::LightGreen => 0x0B,
        Color::LightYellow => 0x0C,
        Color::LightBlue => 0x0D,
        Color::LightMagenta => 0x0E,
        Color::LightCyan => 0x0F,
        Color::White => 0x10,
        Color::Indexed(index) => 0x01_00_00_00 | u32::from(index),
        Color::Rgb(r, g, b) => u32::from_be_bytes([0x02, r, g, b]),
    }
}

/// Convert one buffer cell, without a hyperlink.
#[must_use]
pub fn cell_from_buffer(cell: &Cell) -> CellData {
    CellData {
        symbol: cell.symbol().to_owned(),
        fg: color_to_u32(cell.fg),
        bg: color_to_u32(cell.bg),
        modifier: cell.modifier.bits(),
        skip: cell.diff_option == CellDiffOption::Skip,
        hyperlink: None,
    }
}

/// Convert a buffer into a frame record in row-major order.
///
/// `hyperlinks` pairs a buffer position with the symbol drawn there and its
/// target; a link applies only while the cell still shows that symbol. Equal
/// targets share one entry in [`FrameData::hyperlinks`].
#[must_use]
pub fn frame_from_buffer(
    buffer: &Buffer,
    cursor: Option<CursorState>,
    hyperlinks: &[((u16, u16), String, String)],
) -> FrameData {
    let (width, height) = (buffer.area.width, buffer.area.height);
    let by_position: HashMap<(u16, u16), (&str, &str)> = hyperlinks
        .iter()
        .map(|(position, symbol, uri)| (*position, (symbol.as_str(), uri.as_str())))
        .collect();
    let mut uris = Vec::<String>::new();
    let mut indices = HashMap::<&str, u32>::new();
    let positions = (0..height).flat_map(|row| (0..width).map(move |col| (col, row)));
    let cells = positions
        .zip(buffer.content())
        .map(|(position, cell)| {
            let mut data = cell_from_buffer(cell);
            data.hyperlink = by_position
                .get(&position)
                .filter(|(symbol, _)| *symbol == cell.symbol())
                .map(|(_, uri)| {
                    *indices.entry(uri).or_insert_with(|| {
                        uris.push((*uri).to_owned());
                        u32::try_from(uris.len() - 1).unwrap_or(u32::MAX)
                    })
                });
            data
        })
        .collect();
    FrameData {
        cells,
        width,
        height,
        cursor,
        hyperlinks: uris,
        graphics: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
