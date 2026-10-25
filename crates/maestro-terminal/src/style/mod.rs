//! Cell styling and colour resolution.

mod vt;

use crate::{
    color::{Palette, Rgb},
    page::{Cell, cell::Content},
    sgr::Underline,
};

/// A page-local style identifier.
pub type Id = u16;
/// The identifier requiring no style-table lookup.
pub const DEFAULT_ID: Id = 0;

/// A colour source retained until palette resolution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    /// The terminal's default colour.
    #[default]
    None,
    /// An indexed colour.
    Palette(u8),
    /// A direct colour.
    Rgb(Rgb),
}

/// Packed text attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flags(u16);

/// The configured foreground treatment for bold text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoldColor {
    /// Replace default-coloured bold text with this colour.
    Color(Rgb),
    /// Brighten low indexed colours only.
    Bright,
}

/// Caller-supplied foreground defaults and palette.
#[derive(Clone, Copy)]
pub struct Fg<'a> {
    /// Default foreground colour.
    pub default: Rgb,
    /// Indexed colour values.
    pub palette: &'a Palette,
    /// Optional bold-colour treatment.
    pub bold: Option<BoldColor>,
}

/// Colours and attributes shared by terminal cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    /// Foreground colour source.
    pub fg_color: Color,
    /// Background colour source.
    pub bg_color: Color,
    /// Underline colour source.
    pub underline_color: Color,
    /// Text attributes.
    pub flags: Flags,
}

impl Style {
    /// Whether no colours or attributes are specified.
    #[must_use]
    pub fn is_default(self) -> bool {
        self == Self::default()
    }

    /// Resolve the background, preferring a background-only cell.
    #[must_use]
    pub fn bg(self, cell: &Cell, palette: &Palette) -> Option<Rgb> {
        match cell.content() {
            Content::BgColorPalette(index) => Some(palette[usize::from(index)]),
            Content::BgColorRgb(rgb) => Some(rgb),
            _ => resolve(self.bg_color, palette),
        }
    }

    /// Resolve the foreground under the supplied bold policy.
    #[must_use]
    pub fn fg(self, opts: Fg<'_>) -> Rgb {
        match self.fg_color {
            Color::None => self.bold_default(opts.default, opts.bold),
            Color::Rgb(rgb) if rgb == opts.default => self.bold_default(rgb, opts.bold),
            Color::Rgb(rgb) => rgb,
            Color::Palette(index) => {
                let index = if self.flags.bold() && opts.bold.is_some() && index < 8 {
                    index + 8
                } else {
                    index
                };
                opts.palette[usize::from(index)]
            }
        }
    }

    /// Resolve the independently supplied underline colour.
    #[must_use]
    pub fn underline_color(self, palette: &Palette) -> Option<Rgb> {
        resolve(self.underline_color, palette)
    }

    /// Construct a background-only cell when a background is supplied.
    #[must_use]
    pub fn bg_cell(self) -> Option<Cell> {
        let content = match self.bg_color {
            Color::None => return None,
            Color::Palette(index) => Content::BgColorPalette(index),
            Color::Rgb(rgb) => Content::BgColorRgb(rgb),
        };
        let mut cell = Cell::default();
        cell.set_content(content);
        Some(cell)
    }

    /// Apply a custom bold colour to a default-coloured foreground.
    fn bold_default(self, default: Rgb, bold: Option<BoldColor>) -> Rgb {
        if let (true, Some(BoldColor::Color(rgb))) = (self.flags.bold(), bold) {
            rgb
        } else {
            default
        }
    }
}

/// Resolve a nonforeground colour source.
fn resolve(color: Color, palette: &Palette) -> Option<Rgb> {
    match color {
        Color::None => None,
        Color::Palette(index) => Some(palette[usize::from(index)]),
        Color::Rgb(rgb) => Some(rgb),
    }
}

impl Flags {
    /// Read the underline decoration.
    #[must_use]
    pub fn underline(self) -> Underline {
        match (self.0 >> 8) & 7 {
            1 => Underline::Single,
            2 => Underline::Double,
            3 => Underline::Curly,
            4 => Underline::Dotted,
            5 => Underline::Dashed,
            _ => Underline::None,
        }
    }

    /// Replace the underline decoration.
    pub fn set_underline(&mut self, value: Underline) {
        self.0 = (self.0 & !(7 << 8)) | (u16::from(value as u8) << 8);
    }

    /// Whether bold text is enabled.
    #[must_use]
    pub fn bold(self) -> bool {
        self.0 & (1 << 0) != 0
    }

    /// Enable or disable bold text.
    pub fn set_bold(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 0)) | u16::from(value);
    }

    /// Whether italic text is enabled.
    #[must_use]
    pub fn italic(self) -> bool {
        self.0 & (1 << 1) != 0
    }

    /// Enable or disable italic text.
    pub fn set_italic(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 1)) | (u16::from(value) << 1);
    }

    /// Whether faint text is enabled.
    #[must_use]
    pub fn faint(self) -> bool {
        self.0 & (1 << 2) != 0
    }

    /// Enable or disable faint text.
    pub fn set_faint(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 2)) | (u16::from(value) << 2);
    }

    /// Whether blink text is enabled.
    #[must_use]
    pub fn blink(self) -> bool {
        self.0 & (1 << 3) != 0
    }

    /// Enable or disable blink text.
    pub fn set_blink(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 3)) | (u16::from(value) << 3);
    }

    /// Whether inverse text is enabled.
    #[must_use]
    pub fn inverse(self) -> bool {
        self.0 & (1 << 4) != 0
    }

    /// Enable or disable inverse text.
    pub fn set_inverse(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 4)) | (u16::from(value) << 4);
    }

    /// Whether invisible text is enabled.
    #[must_use]
    pub fn invisible(self) -> bool {
        self.0 & (1 << 5) != 0
    }

    /// Enable or disable invisible text.
    pub fn set_invisible(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 5)) | (u16::from(value) << 5);
    }

    /// Whether strikethrough text is enabled.
    #[must_use]
    pub fn strikethrough(self) -> bool {
        self.0 & (1 << 6) != 0
    }

    /// Enable or disable strikethrough text.
    pub fn set_strikethrough(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 6)) | (u16::from(value) << 6);
    }

    /// Whether overline text is enabled.
    #[must_use]
    pub fn overline(self) -> bool {
        self.0 & (1 << 7) != 0
    }

    /// Enable or disable overline text.
    pub fn set_overline(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 7)) | (u16::from(value) << 7);
    }
}

#[cfg(test)]
mod tests;

/// Deduplicated style ownership within one page.
pub(crate) type Set = crate::ref_counted_set::RefCountedSet<Style>;
