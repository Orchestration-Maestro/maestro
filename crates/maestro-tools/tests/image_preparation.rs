//! Image preparation through public utility operations.

use crate::support;
use support::TestResult;

use base64::Engine;
use maestro_tools::utils::image_convert::convert_to_png;
use maestro_tools::utils::image_resize::{
    ImageResizeOptions, ResizedImage, format_dimension_note, resize_image,
};
use serde::Deserialize;
use support::cases;

/// Measured maximum JPEG channel difference across the archived eighteen-image corpus.
const JPEG_CHANNEL_TOLERANCE: u8 = 3;

/// Encoded bytes with a MIME label.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversionInput {
    /// Authored base64 text.
    pub data: String,
    /// Authored MIME label.
    pub mime_type: String,
}

/// Compare records exactly, allowing native JPEG decoding's channel rounding.
fn check(name: &str) -> TestResult {
    for case in cases("image_preparation", name)? {
        let jpeg = jpeg_is_reencoded(&case.operation, &case.input, &case.expected)?;
        let (mut actual, mut expected) = observe(&case.operation, case.input, case.expected)?;
        if jpeg {
            compare_pixels(&actual["data"], &expected["data"], &case.id)?;
            actual
                .as_object_mut()
                .ok_or("missing result")?
                .shift_remove("data");
            expected
                .as_object_mut()
                .ok_or("missing reference")?
                .shift_remove("data");
        }
        assert_eq!(actual, expected, "{}", case.id);
    }
    Ok(())
}

/// Identify JPEG byte inputs only when the operation produces new encoded data.
fn jpeg_is_reencoded(
    operation: &str,
    input: &serde_json::Value,
    expected: &serde_json::Value,
) -> TestResult<bool> {
    let image = if operation == "resize" {
        &input["image"]
    } else {
        input
    };
    if operation != "convert" && expected["wasResized"] != true {
        return Ok(false);
    }
    let Some(data) = image["data"].as_str() else {
        return Ok(false);
    };
    // Failed decoding and PNG passthrough retain their exact observations.
    if expected.is_null() || image["mimeType"] == "image/png" && operation == "convert" {
        return Ok(false);
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    Ok(bytes.starts_with(&[0xff, 0xd8]))
}

/// Check output format, geometry and every RGBA channel against recorded pixels.
fn compare_pixels(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    id: &str,
) -> TestResult {
    let actual =
        base64::engine::general_purpose::STANDARD.decode(actual.as_str().ok_or("data")?)?;
    let expected = base64::engine::general_purpose::STANDARD
        .decode(expected.as_str().ok_or("reference data")?)?;
    assert_eq!(
        image::guess_format(&actual)?,
        image::guess_format(&expected)?,
        "{id}"
    );
    let actual = image::load_from_memory(&actual)?.to_rgba8();
    let expected = image::load_from_memory(&expected)?.to_rgba8();
    assert_eq!(actual.dimensions(), expected.dimensions(), "{id}");
    for (channel, (actual, expected)) in actual.as_raw().iter().zip(expected.as_raw()).enumerate() {
        assert!(
            actual.abs_diff(*expected) <= JPEG_CHANNEL_TOLERANCE,
            "{id}: channel {channel}: {actual} != {expected}"
        );
    }
    Ok(())
}

/// Dispatch a recorded operation without projecting its output.
fn observe(
    operation: &str,
    input: serde_json::Value,
    expected: serde_json::Value,
) -> TestResult<(serde_json::Value, serde_json::Value)> {
    let actual = match operation {
        "convert" => {
            let input: ConversionInput = serde_json::from_value(input)?;
            serde_json::to_value(convert_to_png(&input.data, &input.mime_type))?
        }
        "resize" => return observe_resize(input, expected),
        "note" => serde_json::to_value(format_dimension_note(
            &serde_json::from_value::<NoteInput>(input)?.into_result(),
        ))?,
        _ => return Err("unexpected operation".into()),
    };
    Ok((actual, expected))
}

/// Compact authored whitespace recipe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    /// Base64 prefix.
    base: String,
    /// Number of appended ASCII spaces.
    spaces: usize,
}

/// Authored image input, including compact whitespace cases.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageInput {
    /// Shared image discriminant when recorded.
    #[serde(rename = "type")]
    kind: Option<String>,
    /// Literal authored bytes.
    data: Option<String>,
    /// Alternate recipe representation.
    #[serde(rename = "data_recipe")]
    data_recipe: Option<Recipe>,
    /// Authored MIME label.
    mime_type: String,
}

/// Optional independent resize policies.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Width bound.
    max_width: Option<u32>,
    /// Height bound.
    max_height: Option<u32>,
    /// Base64 byte bound.
    max_bytes: Option<usize>,
    /// First JPEG quality.
    jpeg_quality: Option<u8>,
}

/// Resize input record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResizeInput {
    /// Authored image.
    image: ImageInput,
    /// Independent limits.
    options: Options,
}

/// Expected retained authored text marker.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedData {
    /// Require exact preservation.
    same_as_input: bool,
}

/// Reconstruct input, consume the recipe marker and compare all result fields.
fn observe_resize(
    input: serde_json::Value,
    mut expected: serde_json::Value,
) -> TestResult<(serde_json::Value, serde_json::Value)> {
    let input: ResizeInput = serde_json::from_value(input)?;
    if let Some(kind) = input.image.kind {
        assert_eq!(kind, "image");
    }
    let data = match (input.image.data, input.image.data_recipe) {
        (Some(data), None) => data,
        (None, Some(recipe)) => recipe.base + &" ".repeat(recipe.spaces),
        _ => return Err("exactly one authored data representation required".into()),
    };
    if expected
        .get("data")
        .is_some_and(serde_json::Value::is_object)
    {
        let marker: RetainedData = serde_json::from_value(expected["data"].clone())?;
        assert!(marker.same_as_input);
        expected["data"] = serde_json::Value::String(data.clone());
    }
    let image = maestro_models::ImageContent {
        data,
        mime_type: input.image.mime_type,
    };
    let options = ImageResizeOptions {
        max_width: input.options.max_width,
        max_height: input.options.max_height,
        max_bytes: input.options.max_bytes,
        jpeg_quality: input.options.jpeg_quality,
    };
    Ok((
        serde_json::to_value(resize_image(&image, options))?,
        expected,
    ))
}

#[test]
fn png_passthrough_retains_exact_input() -> TestResult {
    check("png_passthrough_retains_exact_input")
}

#[test]
fn jpeg_conversion_decodes_complete_png() -> TestResult {
    check("jpeg_conversion_decodes_complete_png")
}

#[test]
fn conversion_applies_orientation_before_png_encoding() -> TestResult {
    check("conversion_applies_orientation_before_png_encoding")
}

#[test]
fn fitting_images_keep_input_and_oriented_dimensions() -> TestResult {
    check("fitting_images_keep_input_and_oriented_dimensions")
}

#[test]
fn resize_dimension_limits_reduce_image() -> TestResult {
    check("resize_dimension_limits_reduce_image")
}

#[test]
fn resize_payload_limit_drives_progressive_dimensions() -> TestResult {
    check("resize_payload_limit_drives_progressive_dimensions")
}

#[test]
fn requested_jpeg_quality_keeps_requested_priority() -> TestResult {
    check("requested_jpeg_quality_keeps_requested_priority")
}

/// Note inputs mirror output fields without giving product output a decoder.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NoteInput {
    /// Authored data field.
    data: String,
    /// Authored MIME field.
    mime_type: String,
    /// Source width.
    original_width: u32,
    /// Source height.
    original_height: u32,
    /// Display width.
    width: u32,
    /// Display height.
    height: u32,
    /// Re-encoding flag.
    was_resized: bool,
}

impl NoteInput {
    /// Move every recorded field to the public note operation.
    fn into_result(self) -> ResizedImage {
        ResizedImage {
            data: self.data,
            mime_type: self.mime_type,
            original_width: self.original_width,
            original_height: self.original_height,
            width: self.width,
            height: self.height,
            was_resized: self.was_resized,
        }
    }
}

#[test]
fn dimension_note_uses_width_scale_and_two_place_rounding() -> TestResult {
    check("dimension_note_uses_width_scale_and_two_place_rounding")
}

#[test]
fn unfittable_image_returns_none_at_one_pixel() -> TestResult {
    check("unfittable_image_returns_none_at_one_pixel")
}

#[test]
fn dimension_note_is_absent_for_unmodified_input() -> TestResult {
    check("dimension_note_is_absent_for_unmodified_input")
}

#[test]
fn resize_defaults_apply_independently() -> TestResult {
    check("resize_defaults_apply_independently")
}

#[test]
fn initial_scaling_preserves_rounding_order() -> TestResult {
    check("initial_scaling_preserves_rounding_order")
}

#[test]
fn thin_images_keep_one_pixel_after_scaling() -> TestResult {
    check("thin_images_keep_one_pixel_after_scaling")
}

#[test]
fn base64_input_and_candidate_caps_are_strict() -> TestResult {
    check("base64_input_and_candidate_caps_are_strict")
}

#[test]
fn invalid_image_data_returns_none() -> TestResult {
    check("invalid_image_data_returns_none")
}

#[test]
fn non_png_mime_does_not_restrict_decodable_payload() -> TestResult {
    check("non_png_mime_does_not_restrict_decodable_payload")
}

#[test]
fn base64_spelling_survives_fitting_input() -> TestResult {
    check("base64_spelling_survives_fitting_input")
}

#[test]
fn non_png_formats_convert_through_decoder() -> TestResult {
    check("non_png_formats_convert_through_decoder")
}

#[test]
fn progressive_fit_keeps_unit_axes() -> TestResult {
    check("progressive_fit_keeps_unit_axes")
}

#[test]
fn encoding_selection_returns_first_fit_not_smallest() -> TestResult {
    check("encoding_selection_returns_first_fit_not_smallest")
}

#[test]
fn wide_image_candidates_respect_jpeg_format_dimensions() -> TestResult {
    check("wide_image_candidates_respect_jpeg_format_dimensions")
}

#[test]
fn ico_bytes_convert_and_resize() -> TestResult {
    check("ico_bytes_convert_and_resize")
}

#[test]
fn pnm_bytes_convert_and_resize() -> TestResult {
    check("pnm_bytes_convert_and_resize")
}

#[test]
fn tiff_bytes_convert_and_resize() -> TestResult {
    check("tiff_bytes_convert_and_resize")
}

#[test]
fn hdr_bytes_convert_and_resize() -> TestResult {
    check("hdr_bytes_convert_and_resize")
}

#[test]
fn dds_bytes_convert_and_resize() -> TestResult {
    check("dds_bytes_convert_and_resize")
}

#[test]
fn farbfeld_bytes_convert_and_resize() -> TestResult {
    check("farbfeld_bytes_convert_and_resize")
}
