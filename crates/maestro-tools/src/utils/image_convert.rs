//! PNG conversion for inline images.

use super::exif_orientation::apply_exif_orientation;
use base64::{
    Engine as _, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig, general_purpose::STANDARD},
};
use image::{DynamicImage, ImageOutputFormat, RgbaImage};
use serde::Serialize;
use std::io::Cursor;

/// Encoded image data and its format.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedImage {
    /// Base64-encoded image bytes.
    pub data: String,
    /// MIME type of the encoded bytes.
    pub mime_type: String,
}

/// Pass exact PNG input through unchanged; decode other MIME labels into PNG.
///
/// Returns `None` when byte decoding fails.
#[must_use]
pub fn convert_to_png(base64_data: &str, mime_type: &str) -> Option<ConvertedImage> {
    let data = if mime_type == "image/png" {
        base64_data.to_owned()
    } else {
        encode_image(
            &DynamicImage::ImageRgba8(decode_image(base64_data)?),
            ImageOutputFormat::Png,
        )?
    };
    Some(ConvertedImage {
        data,
        mime_type: "image/png".to_owned(),
    })
}

/// Decode padded or unpadded base64 with URL-safe spelling and ASCII whitespace.
pub(super) fn decode_image(data: &str) -> Option<RgbaImage> {
    let normalized: String = data
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    let config = GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let bytes = GeneralPurpose::new(&alphabet::STANDARD, config)
        .decode(normalized)
        .ok()?;
    let image = decode_bytes(&bytes)?;
    Some(apply_exif_orientation(image, &bytes))
}

/// Encode RGBA pixels in the requested format as base64.
pub(super) fn encode_image(image: &DynamicImage, format: ImageOutputFormat) -> Option<String> {
    Some(STANDARD.encode(encode_bytes(image, format)?))
}

/// Converts raw image bytes to PNG without applying metadata orientation.
#[must_use]
pub fn convert_bytes_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    encode_bytes(
        &DynamicImage::ImageRgba8(decode_bytes(bytes)?),
        ImageOutputFormat::Png,
    )
}

/// Decodes raw pixels without metadata transformation.
fn decode_bytes(bytes: &[u8]) -> Option<RgbaImage> {
    Some(image::load_from_memory(bytes).ok()?.to_rgba8())
}

/// Encodes pixels into owned bytes.
fn encode_bytes(image: &DynamicImage, format: ImageOutputFormat) -> Option<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).ok()?;
    Some(bytes.into_inner())
}
