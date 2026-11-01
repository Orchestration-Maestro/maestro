//! Stored credential records and configured-authentication metadata.
use crate::oauth::OAuthCredentials;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A stored key: a literal, an environment variable name or a `!` command.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ApiKeyCredential {
    /// Configured value resolved when a request key is selected.
    pub key: String,
    /// Stored provider account variables, resolved independently of the request key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, String>>,
}

/// OAuth tokens stored under the `oauth` discriminator.
pub type OAuthCredential = OAuthCredentials;

/// One provider's stored credential.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum AuthCredential {
    /// Stored as `{"type": "api_key", "key": ...}`.
    #[serde(rename = "api_key")]
    ApiKey(ApiKeyCredential),
    /// Stored as `{"type": "oauth", "refresh": ..., "access": ..., "expires": ...}`
    /// followed by provider fields.
    #[serde(rename = "oauth")]
    OAuth(OAuthCredential),
}

/// Typed credentials by provider, in stored order.
pub type AuthStorageData = IndexMap<String, AuthCredential>;

/// Where a provider's configured authentication comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthSource {
    /// A stored record.
    Stored,
    /// A runtime key override.
    Runtime,
    /// A populated provider environment variable.
    Environment,
    /// The fallback resolver.
    Fallback,
    /// A literal key from the model configuration file.
    ModelsJsonKey,
    /// A command key from the model configuration file.
    ModelsJsonCommand,
}

/// Configured-authentication metadata that never carries key or token values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthStatus {
    /// Whether the selected source is a stored record.
    pub configured: bool,
    /// The selected source; absent when nothing is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AuthSource>,
    /// Display label of the source, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}
