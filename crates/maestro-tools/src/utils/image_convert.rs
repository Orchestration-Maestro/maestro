//! PNG conversion for inline images.

use super::photon::decode_image;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;

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
        STANDARD.encode(decode_image(base64_data)?.get_bytes())
    };
    Some(ConvertedImage {
        data,
        mime_type: "image/png".to_owned(),
    })
}
