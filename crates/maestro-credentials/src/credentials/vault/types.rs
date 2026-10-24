//! Stored credential records, their stored JSON form and status metadata.
use indexmap::IndexMap;
use maestro_models::OAuthCredentials;
use serde::Deserialize;
use serde_json::{Map, Value};

/// A stored key: a literal, an environment variable name or a `!` command.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ApiKeyCredential {
    /// Configured value resolved when a request key is selected.
    pub key: String,
}

/// OAuth tokens stored under the `oauth` discriminator.
pub type OAuthCredential = OAuthCredentials;

/// One provider's stored credential.
#[derive(Clone, Debug, PartialEq, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthStatus {
    /// Whether the selected source is a stored record.
    pub configured: bool,
    /// The selected source; absent when nothing is configured.
    pub source: Option<AuthSource>,
    /// Display label of the source, when it has one.
    pub label: Option<String>,
}

/// A recorded read, lock, parse or write failure.
pub type AuthStorageError = Box<dyn std::error::Error + Send + Sync>;

/// Parse stored text; missing or empty text is an empty document.
pub(super) fn parse_storage_data(
    content: Option<&str>,
) -> Result<Map<String, Value>, serde_json::Error> {
    match content {
        None | Some("") => Ok(Map::new()),
        Some(text) => serde_json::from_str(text),
    }
}

/// Decode one stored object when it is a complete known credential.
pub(super) fn decode(record: &Value) -> Option<AuthCredential> {
    match record {
        Value::Object(_) => AuthCredential::deserialize(record).ok(),
        _ => None,
    }
}

/// The stored form of a typed credential, discriminator first.
pub(super) fn stored_record(credential: AuthCredential) -> Value {
    let mut record = Map::new();
    match credential {
        AuthCredential::ApiKey(api_key) => {
            record.insert("type".into(), "api_key".into());
            record.insert("key".into(), api_key.key.into());
        }
        AuthCredential::OAuth(oauth) => {
            record.insert("type".into(), "oauth".into());
            record.insert("refresh".into(), oauth.refresh.into());
            record.insert("access".into(), oauth.access.into());
            record.insert("expires".into(), stored_number(oauth.expires));
            record.extend(oauth.extra);
        }
    }
    Value::Object(record)
}

/// Integral times keep their integer spelling in the stored document.
fn stored_number(number: f64) -> Value {
    format!("{number}")
        .parse::<i64>()
        .map_or_else(|_| Value::from(number), Value::from)
}
