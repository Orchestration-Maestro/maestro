//! Header inspection and exact cell geometry.
use super::{CellDimensions, ImageDimensions};

/// Returns the aspect-scaled row ceiling, or absence for unusable geometry.
#[must_use]
pub fn calculate_image_rows(
    dimensions: ImageDimensions,
    target_width_cells: u32,
    cells: Option<CellDimensions>,
) -> Option<usize> {
    let cells = cells.unwrap_or_default();
    if dimensions.width_px == 0
        || dimensions.height_px == 0
        || target_width_cells == 0
        || cells.width_px == 0
        || cells.height_px == 0
    {
        return None;
    }
    let numerator = u128::from(dimensions.height_px)
        * u128::from(target_width_cells)
        * u128::from(cells.width_px);
    let denominator = u128::from(dimensions.width_px) * u128::from(cells.height_px);
    usize::try_from(numerator.div_ceil(denominator)).ok()
}

/// Decodes normalized standard or URL-safe base64, retaining permissive trailing bits.
fn decode(data: &str) -> Option<Vec<u8>> {
    use base64::{
        Engine as _, alphabet,
        engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
    };
    let normalized: String = data
        .chars()
        .filter(|value| !value.is_whitespace())
        .map(|value| match value {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    let config = GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    GeneralPurpose::new(&alphabet::STANDARD, config)
        .decode(normalized)
        .ok()
}
/// Rejects zero header geometry.
fn positive(width_px: u32, height_px: u32) -> Option<ImageDimensions> {
    (width_px > 0 && height_px > 0).then_some(ImageDimensions {
        width_px,
        height_px,
    })
}
/// Reads big-endian PNG geometry after its shallow four-byte signature check.
#[must_use]
pub fn get_png_dimensions(base64_data: &str) -> Option<ImageDimensions> {
    let data = decode(base64_data)?;
    if data.len() < 24 || data.get(..4)? != b"\x89PNG" {
        return None;
    }
    positive(
        u32::from_be_bytes(data.get(16..20)?.try_into().ok()?),
        u32::from_be_bytes(data.get(20..24)?.try_into().ok()?),
    )
}

/// Reads SOF0/1/2 geometry from bounded JPEG segments.
#[must_use]
pub fn get_jpeg_dimensions(base64_data: &str) -> Option<ImageDimensions> {
    let data = decode(base64_data)?;
    if data.get(..2)? != b"\xff\xd8" {
        return None;
    }
    let mut offset = 2;
    while offset < data.len().saturating_sub(9) {
        if data[offset] != 0xff {
            offset += 1;
            continue;
        }
        let marker = *data.get(offset + 1)?;
        let length = usize::from(u16::from_be_bytes(
            data.get(offset + 2..offset + 4)?.try_into().ok()?,
        ));
        let end = offset.checked_add(2)?.checked_add(length)?;
        if length < 2 || end > data.len() {
            return None;
        }
        if matches!(marker, 0xc0..=0xc2) {
            if length < 8 {
                return None;
            }
            let height = u16::from_be_bytes(data.get(offset + 5..offset + 7)?.try_into().ok()?);
            let width = u16::from_be_bytes(data.get(offset + 7..offset + 9)?.try_into().ok()?);
            return positive(u32::from(width), u32::from(height));
        }
        offset = end;
    }
    None
}

/// Reads exact GIF87a/GIF89a header geometry.
#[must_use]
pub fn get_gif_dimensions(base64_data: &str) -> Option<ImageDimensions> {
    let data = decode(base64_data)?;
    if !matches!(data.get(..6)?, b"GIF87a" | b"GIF89a") {
        return None;
    }
    positive(
        u32::from(u16::from_le_bytes(data.get(6..8)?.try_into().ok()?)),
        u32::from(u16::from_le_bytes(data.get(8..10)?.try_into().ok()?)),
    )
}

/// Reads VP8, VP8L or VP8X header geometry without decoding image contents.
#[must_use]
pub fn get_webp_dimensions(base64_data: &str) -> Option<ImageDimensions> {
    let data = decode(base64_data)?;
    if data.get(..4)? != b"RIFF" || data.get(8..12)? != b"WEBP" {
        return None;
    }
    let (width, height) = match data.get(12..16)? {
        b"VP8 " => (
            u32::from(u16::from_le_bytes(data.get(26..28)?.try_into().ok()?) & 0x3fff),
            u32::from(u16::from_le_bytes(data.get(28..30)?.try_into().ok()?) & 0x3fff),
        ),
        b"VP8L" => {
            let bits = u32::from_le_bytes(data.get(21..25)?.try_into().ok()?);
            ((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1)
        }
        b"VP8X" => {
            let width = data.get(24..27)?;
            let height = data.get(27..30)?;
            (
                u32::from_le_bytes([width[0], width[1], width[2], 0]) + 1,
                u32::from_le_bytes([height[0], height[1], height[2], 0]) + 1,
            )
        }
        _ => return None,
    };
    positive(width, height)
}

/// Dispatches exact PNG, JPEG, GIF and WebP MIME literals to header readers.
#[must_use]
pub fn get_image_dimensions(base64_data: &str, mime_type: &str) -> Option<ImageDimensions> {
    match mime_type {
        "image/png" => get_png_dimensions(base64_data),
        "image/jpeg" => get_jpeg_dimensions(base64_data),
        "image/gif" => get_gif_dimensions(base64_data),
        "image/webp" => get_webp_dimensions(base64_data),
        _ => None,
    }
}
