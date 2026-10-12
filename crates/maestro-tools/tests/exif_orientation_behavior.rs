//! Metadata orientation through the public image operation.

mod support;
use support::TestResult;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::RgbaImage;
use maestro_tools::utils::exif_orientation::apply_exif_orientation;
use serde::Deserialize;
use serde_json::json;

/// A complete orientation input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Container bytes.
    bytes: String,
    /// Pixel width.
    width: u32,
    /// Pixel height.
    height: u32,
    /// Distinguishable RGBA pixels.
    pixels: Vec<u8>,
}

/// Compare ordered pixels and dimensions after public orientation.
fn check(name: &str) -> TestResult {
    for case in support::cases("exif_orientation_behavior", name)? {
        assert_eq!(case.operation, "orient", "{}", case.id);
        let input: Input = serde_json::from_value(case.input)?;
        let bytes = STANDARD.decode(input.bytes)?;
        let image = RgbaImage::from_raw(input.width, input.height, input.pixels)
            .ok_or("invalid fixture pixels")?;
        let image = apply_exif_orientation(image, &bytes);
        assert_eq!(
            json!({"width":image.width(), "height":image.height(),
            "pixels":image.as_raw()}),
            case.expected,
            "{}",
            case.id
        );
    }
    Ok(())
}

#[test]
fn all_orientations_preserve_rgba_pixel_identity() -> TestResult {
    check("all_orientations_preserve_rgba_pixel_identity")
}

#[test]
fn webp_exif_supports_prefix_padding_and_byte_order() -> TestResult {
    check("webp_exif_supports_prefix_padding_and_byte_order")
}

#[test]
fn exif_boundaries_distinguish_complete_and_unreadable_orientation() -> TestResult {
    check("exif_boundaries_distinguish_complete_and_unreadable_orientation")
}

#[test]
fn non_jpeg_webp_metadata_is_ignored() -> TestResult {
    check("non_jpeg_webp_metadata_is_ignored")
}

#[test]
fn jpeg_xmp_does_not_hide_later_exif() -> TestResult {
    check("jpeg_xmp_does_not_hide_later_exif")
}

#[test]
fn jpeg_exif_skips_unrelated_segments() -> TestResult {
    check("jpeg_exif_skips_unrelated_segments")
}

#[test]
fn first_orientation_survives_duplicates_and_later_metadata_errors() -> TestResult {
    check("first_orientation_survives_duplicates_and_later_metadata_errors")
}
