//! Inbound completion records admitted before reduction.
use super::wire::{SafeInteger, optional};
use crate::providers::json_text::required;
use serde::Deserialize;

/// An admitted complete event.
#[derive(Deserialize)]
pub(super) struct Chunk {
    /// Response identity.
    pub id: String,
    /// All choices, in wire order.
    pub choices: Vec<Record<Choice>>,
    /// Required server model, validated but not used as requested identity.
    #[serde(rename = "model")]
    _model: String,
    /// Optional server object label.
    #[serde(default, rename = "object", deserialize_with = "optional")]
    _object: Option<String>,
    /// Optional creation timestamp.
    #[serde(default, rename = "created", deserialize_with = "optional")]
    _created: Option<SafeInteger>,
    /// Optional reported usage.
    #[serde(default, deserialize_with = "optional")]
    pub usage: Option<Record<Usage>>,
}
/// One selected response choice.
#[derive(Deserialize)]
pub(super) struct Choice {
    /// Required choice index, independent of array selection.
    #[serde(rename = "index")]
    _index: SafeInteger,
    /// Changed message fields.
    #[serde(deserialize_with = "required")]
    pub delta: Delta,
    /// Present nullable finish reason.
    #[serde(deserialize_with = "super::wire::required_nullable")]
    pub finish_reason: Option<String>,
}
/// Incoming message changes.
#[derive(Deserialize)]
pub(super) struct Delta {
    /// Plain text content.
    #[serde(default)]
    pub content: super::content::Contents,
    /// Optional nullable tool updates.
    pub tool_calls: Option<Vec<Record<Tool>>>,
    /// Optional nullable role, eagerly validated.
    #[serde(rename = "role")]
    _role: Option<String>,
    /// Optional nullable tool result identity.
    #[serde(rename = "tool_call_id")]
    _tool_call_id: Option<String>,
    /// Optional nullable message index.
    #[serde(rename = "index")]
    _index: Option<SafeInteger>,
    /// Optional nullable object metadata, without materializing its values.
    #[serde(rename = "metadata")]
    _metadata: Option<Metadata>,
}

/// Reported counts with missing-only defaults.
#[derive(Deserialize)]
pub(super) struct Usage {
    /// Input count.
    #[serde(default)]
    pub prompt_tokens: SafeInteger,
    /// Output count.
    #[serde(default)]
    pub completion_tokens: SafeInteger,
    /// Reported total or zero fallback.
    #[serde(default)]
    pub total_tokens: SafeInteger,
    /// Optional nullable audio duration.
    #[serde(rename = "prompt_audio_seconds")]
    _prompt_audio_seconds: Option<SafeInteger>,
}

/// A fully checked tool update.
#[derive(Deserialize)]
pub(super) struct Tool {
    /// Optional nonnullable open tool kind.
    #[serde(default, rename = "type", deserialize_with = "optional")]
    _kind: Option<String>,
    /// Defaulted identity.
    #[serde(default = "default_id")]
    pub id: String,
    /// Defaulted association index.
    #[serde(default)]
    pub index: SafeInteger,
    /// Required function update.
    pub function: Record<Function>,
}
/// Function name and raw argument representation.
#[derive(Deserialize)]
pub(super) struct Function {
    /// Function name, retained only on creation.
    pub name: String,
    /// Validated string or object dictionary.
    pub arguments: Arguments,
}
/// Default literal identity that requests fallback derivation.
fn default_id() -> String {
    "null".into()
}
/// Raw dictionary or owned argument text.
pub(super) enum Arguments {
    /// Argument text fragment.
    Text(String),
    /// Dictionary awaiting compact serialization.
    Object(Box<serde_json::value::RawValue>),
}
impl<'de> Deserialize<'de> for Arguments {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let raw = Box::<serde_json::value::RawValue>::deserialize(decoder)?;
        if raw.get().starts_with('{') {
            return Ok(Self::Object(raw));
        }
        serde_json::from_str(raw.get())
            .map(Self::Text)
            .map_err(D::Error::custom)
    }
}

/// An object-only record with last-member duplicate resolution.
pub(super) struct Record<T>(pub T);
impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Record<T> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        required(decoder).map(Self)
    }
}
/// Validated object metadata whose members are left unread.
struct Metadata;
impl<'de> Deserialize<'de> for Metadata {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let raw = <&serde_json::value::RawValue>::deserialize(decoder)?;
        if !raw.get().starts_with('{') {
            return Err(D::Error::custom("expected an object"));
        }
        Ok(Self)
    }
}
