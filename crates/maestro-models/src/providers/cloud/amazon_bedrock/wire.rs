//! The service input records a conversation is made of. Every member is optional and absent
//! members are left out on encoding, so a record holds whatever mix of members a payload hook
//! returns and an empty one stays `{}`; members a record does not declare are dropped on decoding.

use base64::Engine as _;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// A list that keeps its present entries, whatever a `null` entry stood for.
pub(super) fn list<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Option::<Vec<Option<T>>>::deserialize(deserializer)?;
    Ok(items.map(|items| items.into_iter().flatten().collect()))
}

/// A map that keeps its present entries, whatever a `null` value stood for.
pub(super) fn map<'de, D, T>(deserializer: D) -> Result<Option<IndexMap<String, T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Option::<IndexMap<String, Option<T>>>::deserialize(deserializer)?;
    Ok(items.map(|items| {
        items
            .into_iter()
            .filter_map(|(name, value)| Some((name, value?)))
            .collect()
    }))
}

/// Image bytes. The payload hook sees them as a number array; the body carries them as text.
pub(super) enum Blob {
    /// Decoded bytes, as the conversion builds them.
    Bytes(Vec<u8>),
    /// Standard padded base64 of the bytes a hook returned.
    Base64(String),
}

impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Bytes(bytes) => bytes.serialize(serializer),
            Self::Base64(text) => serializer.serialize_str(text),
        }
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        Ok(Self::Base64(
            base64::engine::general_purpose::STANDARD.encode(bytes),
        ))
    }
}

/// Where an image's bytes come from.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ImageSource {
    /// The bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bytes: Option<Blob>,
}

/// An image.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ImageBlock {
    /// Where the bytes come from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<ImageSource>,
    /// Image format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) format: Option<String>,
}

/// A reasoning text with the signature that vouches for it.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ReasoningTextBlock {
    /// The text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// The signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) signature: Option<String>,
}

/// A reasoning member of a content union.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ReasoningContentBlock {
    /// Reasoning text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning_text: Option<ReasoningTextBlock>,
}

/// A tool call of the assistant.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolUseBlock {
    /// Call identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_use_id: Option<String>,
    /// Tool name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    /// Arguments, any JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input: Option<Value>,
}

/// A text or image member of a tool result.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ToolResultContentBlock {
    /// Text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// Image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) image: Option<ImageBlock>,
}

/// The result of one tool call.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolResultBlock {
    /// Call identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_use_id: Option<String>,
    /// Result content.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) content: Option<Vec<ToolResultContentBlock>>,
    /// `success` or `error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) status: Option<String>,
}

/// A cache point.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct CachePointBlock {
    /// Cache point kind.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub(super) kind: Option<String>,
    /// Cache lifetime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ttl: Option<String>,
}

/// One member of a turn's content.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ContentBlock {
    /// Text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// Image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) image: Option<ImageBlock>,
    /// Tool call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_use: Option<ToolUseBlock>,
    /// Tool result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_result: Option<ToolResultBlock>,
    /// Reasoning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning_content: Option<ReasoningContentBlock>,
    /// Cache point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cache_point: Option<CachePointBlock>,
}

/// One member of the system prompt.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct SystemContentBlock {
    /// Text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// Cache point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cache_point: Option<CachePointBlock>,
}

/// The shape of a tool's arguments.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ToolInputSchema {
    /// JSON schema, any JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) json: Option<Value>,
}

/// A tool declaration.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolSpecification {
    /// Tool name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    /// What the tool does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) description: Option<String>,
    /// Shape of the arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input_schema: Option<ToolInputSchema>,
}

/// One member of the tool list.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Tool {
    /// The declaration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_spec: Option<ToolSpecification>,
}

/// A tool choice with nothing to say beyond its kind.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Unit {}

/// The choice of one named tool.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct NamedToolChoice {
    /// Tool name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
}

/// Which tool the model must call.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ToolChoiceBlock {
    /// The model decides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) auto: Option<Unit>,
    /// Some tool must be called.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) any: Option<Unit>,
    /// The named tool must be called.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool: Option<NamedToolChoice>,
}

/// Tool declarations with the choice among them.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolConfiguration {
    /// Declared tools.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) tools: Option<Vec<Tool>>,
    /// Tool choice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_choice: Option<ToolChoiceBlock>,
}
