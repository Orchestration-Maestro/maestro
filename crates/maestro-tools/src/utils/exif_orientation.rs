//! JPEG and WebP metadata orientation for decoded RGBA pixels.

use exif::{Exif, In, Reader, Tag};
use image::{
    RgbaImage,
    imageops::{flip_horizontal_in_place, flip_vertical_in_place, rotate90, rotate270},
};
use std::io::Cursor;

/// Apply the first primary JPEG/WebP orientation retained by the metadata reader.
/// A retained unreadable or out-of-range value leaves pixels unchanged.
#[must_use]
pub fn apply_exif_orientation(mut image: RgbaImage, original_bytes: &[u8]) -> RgbaImage {
    let value = metadata(original_bytes)
        .as_ref()
        .and_then(|exif| {
            exif.fields()
                .find(|field| field.tag == Tag::Orientation && field.ifd_num == In::PRIMARY)
        })
        .and_then(|field| field.value.get_uint(0));
    match value {
        Some(2) => flip_horizontal_in_place(&mut image),
        Some(3) => {
            flip_horizontal_in_place(&mut image);
            flip_vertical_in_place(&mut image);
        }
        Some(4) => flip_vertical_in_place(&mut image),
        Some(5) => {
            image = rotate90(&image);
            flip_horizontal_in_place(&mut image);
        }
        Some(6) => image = rotate90(&image),
        Some(7) => {
            image = rotate270(&image);
            flip_horizontal_in_place(&mut image);
        }
        Some(8) => image = rotate270(&image),
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
