//! Shared OAuth credential and prompt records.
use crate::JsonObject;
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

/// Text requested during authorization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthPrompt {
    /// Prompt text.
    pub message: String,
    /// Optional input hint; absent hints are omitted during serialization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Optional permission for empty input; explicit false is retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_empty: Option<bool>,
}

/// Authorization URL with optional instructions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OAuthAuthInfo {
    /// Supplied authorization URL.
    pub url: String,
    /// Optional instructions; absent instructions are omitted during serialization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Open provider identifier.
pub type OAuthProviderId = String;
/// Provider identifier used by authorization records.
pub type OAuthProvider = OAuthProviderId;

/// One supplied authorization choice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OAuthSelectOption {
    /// Open choice identifier.
    pub id: String,
    /// Display label.
    pub label: String,
}

/// Choice prompt retaining supplied option order and repeated identifiers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OAuthSelectPrompt {
    /// Prompt text.
    pub message: String,
    /// Supplied choices in order.
    pub options: Vec<OAuthSelectOption>,
}
