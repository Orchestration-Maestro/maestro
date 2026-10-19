//! Host terminal colour queries and OSC reply parsing.

/// An eight-bit RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbColor {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// The terminal's default text or background colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultColorKind {
    /// Default text colour.
    Foreground,
    /// Default background colour.
    Background,
}

/// Parse a BEL- or ST-terminated OSC 10 or 11 colour reply.
#[must_use]
pub fn parse_default_color_response(sequence: &str) -> Option<(DefaultColorKind, RgbColor)> {
    let body = osc_body(sequence)?;
    let (command, value) = body.split_once(';')?;
    let kind = match command {
        "10" => DefaultColorKind::Foreground,
        "11" => DefaultColorKind::Background,
        _ => return None,
    };
    Some((kind, parse_rgb_color(value)?))
}

/// Decode a slash-separated or packed hexadecimal RGB operand.
fn parse_rgb_color(value: &str) -> Option<RgbColor> {
    if let Some(hex) = value.strip_prefix('#') {
        let digits = hex.len() / 3;
        if !(1..=4).contains(&digits) || hex.len() != digits * 3 {
            return None;
        }
        return Some(RgbColor {
            r: parse_hex_component(hex.get(..digits)?)?,
            g: parse_hex_component(hex.get(digits..digits * 2)?)?,
            b: parse_hex_component(hex.get(digits * 2..)?)?,
        });
    }
    let rgb = value
        .strip_prefix("rgb:")
        .or_else(|| value.strip_prefix("rgba:"))?;
    let mut parts = rgb.split('/');
    let color = RgbColor {
        r: parse_hex_component(parts.next()?)?,
        g: parse_hex_component(parts.next()?)?,
        b: parse_hex_component(parts.next()?)?,
    };
    parts.next().is_none().then_some(color)
}

/// Scale one to four hexadecimal digits to eight bits, rounding to nearest.
fn parse_hex_component(component: &str) -> Option<u8> {
    if component.is_empty()
        || component.len() > 4
        || !component.bytes().all(|ch| ch.is_ascii_hexdigit())
    {
        return None;
    }
    let value = u32::from_str_radix(component, 16).ok()?;
    let max = (1u32 << (component.len() * 4)) - 1;
    u8::try_from((value * 255 + max / 2) / max).ok()
}

/// Queries for the default foreground and background colours.
pub const HOST_COLOR_QUERY_SEQUENCE: &str = "\x1b]10;?\x1b\\\x1b]11;?\x1b\\";

/// Build default-colour queries, optionally followed by all 256 palette queries.
#[must_use]
pub fn host_terminal_theme_query_sequence(include_palette: bool) -> String {
    use std::fmt::Write as _;
    let mut sequence = String::from(HOST_COLOR_QUERY_SEQUENCE);
    if include_palette {
        for index in 0..=u8::MAX {
            let _ = write!(sequence, "\x1b]4;{index};?\x1b\\");
        }
    }
    sequence
}

/// Parse a BEL- or ST-terminated OSC 4 palette colour reply.
#[must_use]
pub fn parse_palette_color_response(sequence: &str) -> Option<(u8, RgbColor)> {
    let body = osc_body(sequence)?.strip_prefix("4;")?;
    let (index, value) = body.split_once(';')?;
    Some((index.parse().ok()?, parse_rgb_color(value)?))
}

/// Remove the OSC introducer and either supported terminator.
fn osc_body(sequence: &str) -> Option<&str> {
    let body = sequence.strip_prefix("\x1b]")?;
    body.strip_suffix("\x1b\\")
        .or_else(|| body.strip_suffix('\u{7}'))
}

#[cfg(test)]
mod tests;
