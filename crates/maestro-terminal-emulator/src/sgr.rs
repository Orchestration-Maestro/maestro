//! Select-graphic-rendition attribute values.

/// The decoration used beneath text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Underline {
    /// No underline.
    #[default]
    None = 0,
    /// One solid line.
    Single = 1,
    /// Two solid lines.
    Double = 2,
    /// A wavy line.
    Curly = 3,
    /// A dotted line.
    Dotted = 4,
    /// A dashed line.
    Dashed = 5,
}

use crate::{
    color::{Name, Rgb},
    parser::SepList,
};

/// Unknown SGR parameters with their full input and selected suffix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unknown<'a> {
    /// Complete parameter input.
    pub full: &'a [u16],
    /// Unrecognised parameter group or remaining suffix.
    pub partial: &'a [u16],
}

/// One decoded text attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute<'a> {
    /// Reset all attributes.
    Unset,
    /// Unrecognised parameters.
    Unknown(Unknown<'a>),
    /// Enable bold.
    Bold,
    /// Clear bold and faint.
    ResetBold,
    /// Enable italic.
    Italic,
    /// Clear italic.
    ResetItalic,
    /// Enable faint.
    Faint,
    /// Select underline decoration.
    Underline(Underline),
    /// Set direct underline colour.
    UnderlineColor(Rgb),
    /// Set indexed underline colour.
    Palette256UnderlineColor(u8),
    /// Restore underline colour.
    ResetUnderlineColor,
    /// Enable overline.
    Overline,
    /// Clear overline.
    ResetOverline,
    /// Enable blinking.
    Blink,
    /// Clear blinking.
    ResetBlink,
    /// Enable inverse colours.
    Inverse,
    /// Clear inverse colours.
    ResetInverse,
    /// Enable invisible text.
    Invisible,
    /// Clear invisible text.
    ResetInvisible,
    /// Enable strike-through.
    Strikethrough,
    /// Clear strike-through.
    ResetStrikethrough,
    /// Set direct foreground.
    DirectColorFg(Rgb),
    /// Set direct background.
    DirectColorBg(Rgb),
    /// Set named background.
    Color8Bg(Name),
    /// Set named foreground.
    Color8Fg(Name),
    /// Restore foreground.
    ResetFg,
    /// Restore background.
    ResetBg,
    /// Set bright named background.
    Color8BrightBg(Name),
    /// Set bright named foreground.
    Color8BrightFg(Name),
    /// Set indexed background.
    Palette256Bg(u8),
    /// Set indexed foreground.
    Palette256Fg(u8),
}

/// An iterator over borrowed SGR parameters and colon positions.
pub struct Parser<'a> {
    /// Complete parameter input.
    pub params: &'a [u16],
    /// Colon positions after parameters.
    pub params_sep: SepList,
    /// Next unread parameter.
    pub idx: usize,
}
impl<'a> Parser<'a> {
    /// Empty input emits one reset attribute.
    pub const EMPTY: Self = Self {
        params: &[],
        params_sep: SepList(0),
        idx: 0,
    };

    /// Return the unknown suffix after consuming an unsupported colon group.
    fn unknown(&self, start: usize, end: usize) -> Attribute<'a> {
        Attribute::Unknown(Unknown {
            full: self.params,
            partial: &self.params[start..end],
        })
    }
}
impl<'a> Iterator for Parser<'a> {
    type Item = Attribute<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.params.len() {
            let empty = self.idx == 0;
            self.idx = self.idx.saturating_add(1);
            return empty.then_some(Attribute::Unset);
        }
        let start = self.idx;
        let code = self.params[start];
        let colon = self.params_sep.is_set(start);
        self.idx += 1;
        if colon && !matches!(code, 4 | 38 | 48 | 58) {
            while self.params_sep.is_set(self.idx) {
                self.idx += 1;
            }
            self.idx += 1;
            return Some(self.unknown(start, self.idx.min(self.params.len())));
        }
        let attribute = match code {
            4 => self.underline(colon),
            38 | 48 | 58 => self.color(code, colon),
            _ => scalar(code),
        };
        Some(attribute.unwrap_or_else(|| self.unknown(start, self.params.len())))
    }
}

/// Decode ordinary scalar attributes and palette ranges.
fn scalar(code: u16) -> Option<Attribute<'static>> {
    use Attribute as A;
    Some(match code {
        0 => A::Unset,
        1 => A::Bold,
        2 => A::Faint,
        3 => A::Italic,
        5 | 6 => A::Blink,
        7 => A::Inverse,
        8 => A::Invisible,
        9 => A::Strikethrough,
        21 => A::Underline(Underline::Double),
        22 => A::ResetBold,
        23 => A::ResetItalic,
        24 => A::Underline(Underline::None),
        25 => A::ResetBlink,
        27 => A::ResetInverse,
        28 => A::ResetInvisible,
        29 => A::ResetStrikethrough,
        30..=37 => A::Color8Fg(Name(low_byte(code - 30))),
        39 => A::ResetFg,
        40..=47 => A::Color8Bg(Name(low_byte(code - 40))),
        49 => A::ResetBg,
        53 => A::Overline,
        55 => A::ResetOverline,
        59 => A::ResetUnderlineColor,
        90..=97 => A::Color8BrightFg(Name(low_byte(code - 82))),
        100..=107 => A::Color8BrightBg(Name(low_byte(code - 92))),
        _ => return None,
    })
}

/// Keep the protocol's low byte without a narrowing cast.
fn low_byte(value: u16) -> u8 {
    value.to_le_bytes()[0]
}

impl<'a> Parser<'a> {
    /// Count the remaining colon separators before the last parameter.
    fn count_colons(&self) -> usize {
        (self.idx..self.params.len().saturating_sub(1))
            .take_while(|&i| self.params_sep.is_set(i))
            .count()
    }
    /// Skip the rest of a malformed colon subparameter group.
    fn consume_colons(&mut self) {
        self.idx += self.count_colons() + 1;
    }
    /// Read one underline style without consuming the next ordinary attribute.
    fn underline(&mut self, colon: bool) -> Option<Attribute<'a>> {
        if !colon {
            return Some(Attribute::Underline(Underline::Single));
        }
        let value = *self.params.get(self.idx)?;
        if self.params_sep.is_set(self.idx) {
            self.consume_colons();
            return None;
        }
        self.idx += 1;
        let style = match value {
            0 => Underline::None,
            2 => Underline::Double,
            3 => Underline::Curly,
            4 => Underline::Dotted,
            5 => Underline::Dashed,
            _ => Underline::Single,
        };
        Some(Attribute::Underline(style))
    }
    /// Decode an indexed or direct colour for the selected attribute target.
    fn color(&mut self, code: u16, colon: bool) -> Option<Attribute<'a>> {
        let slice = &self.params[self.idx - 1..];
        match slice.get(1)? {
            5 if slice.len() >= 3 => {
                self.idx += 2;
                let value = low_byte(slice[2]);
                Some(match code {
                    38 => Attribute::Palette256Fg(value),
                    48 => Attribute::Palette256Bg(value),
                    _ => Attribute::Palette256UnderlineColor(value),
                })
            }
            2 if slice.len() >= 5 => self.direct(code, colon),
            _ => None,
        }
    }
    /// Decode three direct channels, ignoring an optional colour-space field.
    fn direct(&mut self, code: u16, colon: bool) -> Option<Attribute<'a>> {
        let offset = if colon {
            match self.count_colons() {
                3 => 1,
                4 => 2,
                _ => {
                    self.consume_colons();
                    return None;
                }
            }
        } else {
            1
        };
        let channels = self.params.get(self.idx + offset..self.idx + offset + 3)?;
        let rgb = Rgb {
            r: low_byte(channels[0]),
            g: low_byte(channels[1]),
            b: low_byte(channels[2]),
        };
        self.idx += offset + 3;
        Some(match code {
            38 => Attribute::DirectColorFg(rgb),
            48 => Attribute::DirectColorBg(rgb),
            _ => Attribute::UnderlineColor(rgb),
        })
    }
}
