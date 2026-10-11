//! JPEG and WebP metadata orientation for decoded RGBA pixels.

use super::photon::PhotonImageType;
use exif::{Exif, In, Reader, Tag};
use photon_rs::transform::{fliph, flipv};
use std::io::Cursor;

/// Apply the first primary JPEG/WebP orientation; unreadable orientation leaves pixels unchanged.
#[must_use]
pub fn apply_exif_orientation(
    mut image: PhotonImageType,
    original_bytes: &[u8],
) -> PhotonImageType {
    let value = metadata(original_bytes)
        .as_ref()
        .and_then(|exif| {
            exif.fields()
                .find(|field| field.tag == Tag::Orientation && field.ifd_num == In::PRIMARY)
        })
        .and_then(|field| field.value.get_uint(0));
    match value {
        Some(2) => fliph(&mut image),
        Some(3) => {
            fliph(&mut image);
            flipv(&mut image);
        }
        Some(4) => flipv(&mut image),
        Some(5) => {
            image = quarter_turn(image, Turn::Clockwise);
            fliph(&mut image);
        }
        Some(6) => image = quarter_turn(image, Turn::Clockwise),
        Some(7) => {
            image = quarter_turn(image, Turn::Counterclockwise);
            fliph(&mut image);
        }
        Some(8) => image = quarter_turn(image, Turn::Counterclockwise),
        _ => {}
    }
    image
}

/// Read JPEG metadata without losing a partial readable orientation.
fn metadata(bytes: &[u8]) -> Option<Exif> {
    let mut reader = Reader::new();
    reader.continue_on_error(true);
    let result = if bytes.starts_with(b"\xff\xd8") {
        reader.read_from_container(&mut Cursor::new(bytes))
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        let raw = webp_exif(bytes)?;
        reader.read_raw(raw.strip_prefix(b"Exif\0\0").unwrap_or(raw).to_vec())
    } else {
        return None;
    };
    result
        .or_else(|error| error.distill_partial_result(|_| {}))
        .ok()
}

/// Find the first EXIF chunk, consuming headers, unsigned payloads and padding monotonically.
fn webp_exif(bytes: &[u8]) -> Option<&[u8]> {
    let mut offset: usize = 12;
    loop {
        let header_end = offset.checked_add(8)?;
        let header = bytes.get(offset..header_end)?;
        let size = usize::try_from(u32::from_le_bytes(header.get(4..8)?.try_into().ok()?)).ok()?;
        let end = header_end.checked_add(size)?;
        let payload = bytes.get(header_end..end)?;
        if header.starts_with(b"EXIF") {
            return Some(payload);
        }
        offset = end.checked_add(size % 2)?;
    }
}

/// Integer quarter-turn direction.
#[derive(Clone, Copy)]
enum Turn {
    /// Rotate right.
    Clockwise,
    /// Rotate left.
    Counterclockwise,
}

/// Permute complete pixels without general-angle resampling.
fn quarter_turn(image: PhotonImageType, turn: Turn) -> PhotonImageType {
    let (Ok(width), Ok(height)) = (
        usize::try_from(image.get_width()),
        usize::try_from(image.get_height()),
    ) else {
        return image;
    };
    let source = image.get_raw_pixels();
    let mut output = vec![0; source.len()];
    for (index, pixel) in source.as_chunks::<4>().0.iter().enumerate() {
        let (x, y) = (index % width, index / width);
        let target = match turn {
            Turn::Clockwise => x * height + (height - 1 - y),
            Turn::Counterclockwise => (width - 1 - x) * height + y,
        };
        output[target * 4..target * 4 + 4].copy_from_slice(pixel);
    }
    PhotonImageType::new(output, image.get_height(), image.get_width())
}
