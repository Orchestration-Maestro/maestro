//! Conversion of authored colors into ANSI prefixes.
use super::{ColorMode, ColorValue, ThemeError};

/// Prepare an authored color for the specified ANSI plane.
pub(super) fn ansi(value: &ColorValue, mode: ColorMode, plane: u8) -> Result<String, ThemeError> {
    match value {
        ColorValue::Index(index) => Ok(format!("\x1b[{plane};5;{index}m")),
        ColorValue::String(value) if value.is_empty() => Ok(format!("\x1b[{}m", plane + 1)),
        ColorValue::String(value) if value.starts_with('#') => {
            let [r, g, b] = rgb(value)?;
            match mode {
                ColorMode::Truecolor => Ok(format!("\x1b[{plane};2;{r};{g};{b}m")),
                ColorMode::Color256 => Ok(format!("\x1b[{plane};5;{}m", palette([r, g, b]))),
            }
        }
        ColorValue::String(value) => {
            Err(ThemeError::message(format!("Invalid color value: {value}")))
        }
    }
}
/// Parse exactly six ASCII hexadecimal digits after the color marker.
fn rgb(value: &str) -> Result<[u8; 3], ThemeError> {
    let hex = &value[1..];
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ThemeError::message(format!("Invalid hex color: {value}")));
    }
    let number = u32::from_str_radix(hex, 16)
        .map_err(|_| ThemeError::message(format!("Invalid hex color: {value}")))?;
    let [_, r, g, b] = number.to_be_bytes();
    Ok([r, g, b])
}

/// Channel intensities of the indexed RGB cube.
const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
/// Select the nearest channel, retaining the lower index on ties.
fn nearest(value: f64, channels: impl Iterator<Item = (u8, u8)>) -> (u8, u8) {
    channels
        .min_by(|(_, a), (_, b)| {
            (value - f64::from(*a))
                .abs()
                .total_cmp(&(value - f64::from(*b)).abs())
        })
        .unwrap_or((0, 0))
}
/// Weighted squared RGB distance in source expression order.
fn distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    let [dr, dg, db] = std::array::from_fn::<_, 3, _>(|i| f64::from(a[i]) - f64::from(b[i]));
    dr * dr * 0.299 + dg * dg * 0.587 + db * db * 0.114
}
/// Quantize RGB using the nearest cube color or an unsaturated gray.
fn palette(rgb: [u8; 3]) -> u8 {
    let cube = rgb.map(|channel| nearest(f64::from(channel), (0..6).zip(CUBE)));
    let cube_rgb = cube.map(|(_, channel)| channel);
    let cube_index = 16 + 36 * cube[0].0 + 6 * cube[1].0 + cube[2].0;
    let [r, g, b] = rgb.map(f64::from);
    let gray = (0.299 * r + 0.587 * g + 0.114 * b).round();
    let (gray_index, gray_value) = nearest(gray, (0..24).map(|i| (i, 8 + i * 10)));
    let spread = rgb.into_iter().max().unwrap_or(0) - rgb.into_iter().min().unwrap_or(0);
    if spread < 10 && distance(rgb, [gray_value; 3]) < distance(rgb, cube_rgb) {
        232 + gray_index
    } else {
        cube_index
    }
}
