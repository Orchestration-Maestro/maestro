//! Content records that events carry.

use serde::{Deserialize, Serialize};

/// A base64 image with its MIME type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    /// The base64 data.
    pub data: String,
    /// The MIME type of the data, such as `image/png`.
    pub mime_type: String,
}
