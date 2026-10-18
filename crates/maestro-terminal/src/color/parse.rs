//! Byte grammars for terminal colours.

use super::{PaletteEntry, ParseError, Rgb};
use num_traits::ToPrimitive;

/// Remove only the protocol's edge spaces and tabs.
pub(super) fn trim(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|b| !matches!(b, b' ' | b'\t'))
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t'))
        .map_or(start, |n| n + 1);
    &value[start..end]
}

/// Decode a palette assignment with a radix-prefixed index.
///
/// # Errors
/// Returns the colour grammar error or an overflowing palette index.
pub fn parse_palette_entry(value: &[u8]) -> Result<PaletteEntry, ParseError> {
    let pos = value
        .iter()
        .position(|&b| b == b'=')
        .ok_or(ParseError::InvalidFormat)?;
    let index = palette_index(trim(&value[..pos]))?;
    Ok(PaletteEntry {
        index,
        color: Rgb::parse(&value[pos + 1..])?,
    })
}

/// Read a signed palette index while preserving negative zero.
fn palette_index(value: &[u8]) -> Result<u8, ParseError> {
    u8::try_from(index(value, 0, 255)?).map_err(|_| ParseError::Overflow)
}

/// Decode a signed nonnegative index with an explicit radix and upper bound.
///
/// A zero radix admits the palette assignment's radix prefixes.
pub(crate) fn index(value: &[u8], radix: u32, max: u16) -> Result<u16, ParseError> {
    let (negative, value) = match value.first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let (radix, digits) = if radix == 0 {
        match value.get(..2) {
            Some(b"0x" | b"0X") => (16, &value[2..]),
            Some(b"0o" | b"0O") => (8, &value[2..]),
            Some(b"0b" | b"0B") => (2, &value[2..]),
            _ => (10, value),
        }
    } else {
        (radix, value)
    };
    if digits.is_empty() || !digits.iter().all(|b| char::from(*b).is_digit(radix)) {
        return Err(ParseError::InvalidFormat);
    }
    let text = std::str::from_utf8(digits).map_err(|_| ParseError::InvalidFormat)?;
    let n = u16::from_str_radix(text, radix).map_err(|_| ParseError::Overflow)?;
    if n > max || (negative && n != 0) {
        return Err(ParseError::Overflow);
    }
    Ok(n)
}

/// Scale one unsigned hexadecimal channel to eight bits.
fn hex(value: &[u8]) -> Result<u8, ParseError> {
    if !(1..=4).contains(&value.len()) || !value.iter().all(u8::is_ascii_hexdigit) {
        return Err(ParseError::InvalidFormat);
    }
    let text = std::str::from_utf8(value).map_err(|_| ParseError::InvalidFormat)?;
    let n = u32::from_str_radix(text, 16).map_err(|_| ParseError::InvalidFormat)?;
    u8::try_from(n * 255 / ((1 << (4 * value.len())) - 1)).map_err(|_| ParseError::InvalidFormat)
}

/// Convert a decimal floating intensity, retaining the NaN-zero result.
fn intensity(value: &[u8]) -> Result<u8, ParseError> {
    if value.iter().any(|b| b.is_ascii_whitespace() || *b == b'_') {
        return Err(ParseError::InvalidFormat);
    }
    let text = std::str::from_utf8(value).map_err(|_| ParseError::InvalidFormat)?;
    let n: f64 = text.parse().map_err(|_| ParseError::InvalidFormat)?;
    if n.is_nan() {
        return Ok(0);
    }
    if !(0.0..=1.0).contains(&n) {
        return Err(ParseError::InvalidFormat);
    }
    (n * 255.0).to_u8().ok_or(ParseError::InvalidFormat)
}

/// Decode three equal-width hexadecimal fields.
fn packed(value: &[u8]) -> Result<Rgb, ParseError> {
    let width = value.len() / 3;
    Ok(Rgb {
        r: hex(&value[..width])?,
        g: hex(&value[width..2 * width])?,
        b: hex(&value[2 * width..])?,
    })
}

impl Rgb {
    /// Decode a hash, slash, intensity or named colour from bytes.
    ///
    /// # Errors
    /// Returns `InvalidFormat` when the bytes do not encode a colour.
    pub fn parse(value: &[u8]) -> Result<Self, ParseError> {
        let input = trim(value);
        if let Some(digits) = input.strip_prefix(b"#") {
            return if matches!(digits.len(), 3 | 6 | 9 | 12) {
                packed(digits)
            } else {
                Err(ParseError::InvalidFormat)
            };
        }
        if let Some(color) = crate::x11_color::map().get(input) {
            return Ok(color);
        }
        if matches!(input.len(), 3 | 6) {
            return packed(input);
        }
        if let Some(body) = input.strip_prefix(b"rgb:") {
            return slash(body, hex);
        }
        if let Some(body) = input.strip_prefix(b"rgbi:") {
            return slash(body, intensity);
        }
        Err(ParseError::InvalidFormat)
    }
}

/// Decode three slash-separated channels using the selected numeric grammar.
fn slash(body: &[u8], channel: fn(&[u8]) -> Result<u8, ParseError>) -> Result<Rgb, ParseError> {
    let mut parts = body.split(|&b| b == b'/');
    let r = channel(parts.next().ok_or(ParseError::InvalidFormat)?)?;
    let g = channel(parts.next().ok_or(ParseError::InvalidFormat)?)?;
    let b = channel(parts.next().ok_or(ParseError::InvalidFormat)?)?;
    if parts.next().is_some() {
        return Err(ParseError::InvalidFormat);
    }
    Ok(Rgb { r, g, b })
}
