//! OAuth token records shared by the native credential owners and extensions.
use crate::types::JsonObject;
use serde::{Deserialize, Serialize};

/// Token data with provider-specific extension fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OAuthCredentials {
    /// Refresh token text.
    pub refresh: String,
    /// Access token text.
    pub access: String,
    /// Expiry in Unix epoch milliseconds; carried without expiry policy.
    pub expires: f64,
    /// Provider fields other than refresh, access and expires.
    #[serde(flatten)]
    pub extra: JsonObject,
}
