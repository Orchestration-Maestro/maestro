//! Self-contained SGR encoding for cell styles.

use super::{Color, Style};
use crate::{color::Palette, sgr::Underline};
use std::fmt::{self, Write};

impl Style {
    /// Write a reset followed by every enabled attribute and colour.
    ///
    /// # Errors
    /// Returns the destination's formatting error.
    pub fn format_vt(self, writer: &mut impl Write, palette: Option<&Palette>) -> fmt::Result {
        writer.write_str("\x1b[0m")?;
        let flags = self.flags;
        for (enabled, code) in [
            (flags.bold(), 1),
            (flags.faint(), 2),
            (flags.italic(), 3),
            (flags.blink(), 5),
            (flags.inverse(), 7),
            (flags.invisible(), 8),
            (flags.strikethrough(), 9),
            (flags.overline(), 53),
        ] {
            if enabled {
                write!(writer, "\x1b[{code}m")?;
            }
        }
        match flags.underline() {
            Underline::None => {}
            Underline::Single => writer.write_str("\x1b[4m")?,
            value => write!(writer, "\x1b[4:{}m", value as u8)?,
        }
        format_color(writer, 38, self.fg_color, palette)?;
        format_color(writer, 48, self.bg_color, palette)?;
        format_color(writer, 58, self.underline_color, palette)
    }
}

/// Encode a direct colour or an optionally resolved palette reference.
fn format_color(
    writer: &mut impl Write,
    prefix: u8,
    color: Color,
    palette: Option<&Palette>,
) -> fmt::Result {
    let rgb = match color {
        Color::None => return Ok(()),
        Color::Rgb(rgb) => rgb,
        Color::Palette(index) => match palette {
            Some(palette) => palette[usize::from(index)],
            None => return write!(writer, "\x1b[{prefix};5;{index}m"),
        },
    };
    write!(writer, "\x1b[{prefix};2;{};{};{}m", rgb.r, rgb.g, rgb.b)
}
