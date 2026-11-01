//! Stored credential records, their stored JSON form and status metadata.
pub use maestro_request::auth_storage::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, AuthStorageData, OAuthCredential,
};
use serde::Deserialize;
use serde_json::{Map, Value};

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
            if let Some(env) = api_key.env {
                record.insert(
                    "env".into(),
                    Value::Object(
                        env.into_iter()
                            .map(|(name, value)| (name, Value::String(value)))
                            .collect(),
                    ),
                );
            }
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
