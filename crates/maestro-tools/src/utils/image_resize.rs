//! Image fitting with independent geometry and base64 payload limits.

use super::{
    image_convert::ConvertedImage,
    photon::{PhotonImageType, decode_image},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use maestro_models::ImageContent;
use num_traits::ToPrimitive;
use photon_rs::transform::{SamplingFilter, resize};
use serde::Serialize;

/// Independent image limits; absent fields select the documented defaults.
#[derive(Clone, Copy, Default)]
pub struct ImageResizeOptions {
    /// Maximum width, default 2000 pixels.
    pub max_width: Option<u32>,
    /// Maximum height, default 2000 pixels.
    pub max_height: Option<u32>,
    /// Strict base64 byte limit, default 4718592.
    pub max_bytes: Option<usize>,
    /// First JPEG quality, default 80.
    pub jpeg_quality: Option<u8>,
}

/// Encoded image with oriented source and displayed dimensions.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizedImage {
    /// Base64 encoded image.
    pub data: String,
    /// Encoded format, or the authored MIME when retained.
    pub mime_type: String,
    /// Oriented source width.
    pub original_width: u32,
    /// Oriented source height.
    pub original_height: u32,
    /// Displayed width.
    pub width: u32,
    /// Displayed height.
    pub height: u32,
    /// Whether encoding was performed, even at unchanged dimensions.
    pub was_resized: bool,
}

/// Retain authored input when oriented dimensions and its strict byte cap fit.
/// Returns `None` for undecodable input or when no candidate fits.
#[must_use]
pub fn resize_image(img: &ImageContent, options: ImageResizeOptions) -> Option<ResizedImage> {
    let image = decode_image(&img.data)?;
    let (width, height) = (image.get_width(), image.get_height());
    let cap = options.max_bytes.unwrap_or(4_718_592);
    let bounds = (
        options.max_width.unwrap_or(2000),
        options.max_height.unwrap_or(2000),
    );
    if width <= bounds.0 && height <= bounds.1 && img.data.len() < cap {
        return Some(ResizedImage {
            data: img.data.clone(),
            mime_type: img.mime_type.clone(),
            original_width: width,
            original_height: height,
            width,
            height,
            was_resized: false,
        });
    }
    let (mut width, mut height) = initial_dimensions((width, height), bounds)?;
    loop {
        let scaled = resize(&image, width, height, SamplingFilter::Lanczos3);
        if let Some(encoded) = fitting_encoding(&scaled, cap, options.jpeg_quality.unwrap_or(80)) {
            return Some(ResizedImage {
                data: encoded.data,
                mime_type: encoded.mime_type,
                original_width: image.get_width(),
                original_height: image.get_height(),
                width,
                height,
                was_resized: true,
            });
        }
        if width == 1 && height == 1 {
            return None;
        }
        width = shrink(width);
        height = shrink(height);
    }
}

/// Try PNG then first-occurrence JPEG qualities, stopping at the first strict fit.
fn fitting_encoding(image: &PhotonImageType, cap: usize, quality: u8) -> Option<ConvertedImage> {
    let data = STANDARD.encode(image.get_bytes());
    if data.len() < cap {
        return Some(ConvertedImage {
            data,
            mime_type: "image/png".to_owned(),
        });
    }
    if image.get_width() > u32::from(u16::MAX) || image.get_height() > u32::from(u16::MAX) {
        return None;
    }
    let qualities = [quality, 85, 70, 55, 40];
    for (index, quality) in qualities.iter().enumerate() {
        if qualities[..index].contains(quality) {
            continue;
        }
        let data = STANDARD.encode(image.get_bytes_jpeg(*quality));
        if data.len() < cap {
            return Some(ConvertedImage {
                data,
                mime_type: "image/jpeg".to_owned(),
            });
        }
    }
    None
}

/// Scale width before height, retaining positive geometry and half-up rounding.
fn initial_dimensions(size: (u32, u32), bounds: (u32, u32)) -> Option<(u32, u32)> {
    if bounds.0 == 0 || bounds.1 == 0 {
        return None;
    }
    let (mut width, mut height) = size;
    if width > bounds.0 {
        height = (f64::from(height) * f64::from(bounds.0) / f64::from(width))
            .round()
            .to_u32()?;
        width = bounds.0;
    }
    if height > bounds.1 {
        width = (f64::from(width) * f64::from(bounds.1) / f64::from(height))
            .round()
            .to_u32()?;
        height = bounds.1;
    }
    Some((width.max(1), height.max(1)))
}

/// Floor the three-quarter dimension, preserving a unit axis.
fn shrink(value: u32) -> u32 {
    (value / 4 * 3 + value % 4 * 3 / 4).max(1)
}

/// Describe coordinate scaling only when the image was re-encoded.
#[must_use]
pub fn format_dimension_note(result: &ResizedImage) -> Option<String> {
    if !result.was_resized {
        return None;
    }
    let scale = f64::from(result.original_width) / f64::from(result.width);
    Some(format!(
        "[Image: original {}x{}, displayed at {}x{}. Multiply coordinates by {} to map to original image.]",
        result.original_width,
        result.original_height,
        result.width,
        result.height,
        ryu_js::Buffer::new().format_to_fixed(scale, 2)
    ))
}
