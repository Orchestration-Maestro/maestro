//! Typed content transport records.

use super::{Nullable, Object, SafeInteger, nullable, optional, optional_literal};
use serde::{Deserialize, Serialize};

/// Transport fields for `TextChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct TextChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<TextChunkTag>,
    /// The `text` field.
    text: String,
}

/// Transport fields for `ThinkChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ThinkChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<ThinkChunkTag>,
    /// The `thinking` field.
    thinking: Vec<ThinkingPart>,
    /// The `signature` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    signature: Option<Nullable<String>>,
    /// The `closed` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    closed: Option<bool>,
}

/// Transport fields for `ToolReferenceChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ToolReferenceChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<ToolReferenceChunkTag>,
    /// The `tool` field.
    tool: String,
    /// The `title` field.
    title: String,
    /// The `url` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    url: Option<Nullable<String>>,
    /// The `favicon` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    favicon: Option<Nullable<String>>,
    /// The `description` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<Nullable<String>>,
}

/// Transport fields for `ReferenceChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ReferenceChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<ReferenceChunkTag>,
    /// The `referenceIds` field.
    reference_ids: Vec<ReferenceId>,
}

/// Transport fields for `ImageURLChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ImageURLChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<ImageURLChunkTag>,
    /// The `imageUrl` field.
    image_url: ImageLocation,
}

/// Transport fields for `ImageURL`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ImageURL {
    /// The `url` field.
    url: String,
    /// The `detail` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    detail: Option<Nullable<String>>,
}

/// Transport fields for `FileChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct FileChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<FileChunkTag>,
    /// The `fileId` field.
    file_id: String,
}

/// Transport fields for `DocumentURLChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct DocumentURLChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<DocumentURLChunkTag>,
    /// The `documentUrl` field.
    document_url: String,
    /// The `documentName` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    document_name: Option<Nullable<String>>,
}

/// Transport fields for `AudioChunk`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct AudioChunk {
    /// The `type` field.
    #[serde(default, deserialize_with = "optional_literal", rename = "type")]
    kind: Option<AudioChunkTag>,
    /// The `inputAudio` field.
    input_audio: String,
}

/// Literal `text` tag.
#[derive(Default, Deserialize, Serialize)]
pub(super) enum TextChunkTag {
    /// Transport discriminator.
    #[default]
    #[serde(rename = "text")]
    Value,
}

/// Literal `thinking` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum ThinkChunkTag {
    /// Transport discriminator.
    #[serde(rename = "thinking")]
    Value,
}

/// Literal `tool_reference` tag.
#[derive(Default, Deserialize, Serialize)]
pub(super) enum ToolReferenceChunkTag {
    /// Transport discriminator.
    #[default]
    #[serde(rename = "tool_reference")]
    Value,
}

/// Literal `reference` tag.
#[derive(Default, Deserialize, Serialize)]
pub(super) enum ReferenceChunkTag {
    /// Transport discriminator.
    #[default]
    #[serde(rename = "reference")]
    Value,
}

/// Literal `image_url` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum ImageURLChunkTag {
    /// Transport discriminator.
    #[serde(rename = "image_url")]
    Value,
}

/// Literal `file` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum FileChunkTag {
    /// Transport discriminator.
    #[serde(rename = "file")]
    Value,
}

/// Literal `document_url` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum DocumentURLChunkTag {
    /// Transport discriminator.
    #[serde(rename = "document_url")]
    Value,
}

/// Literal `input_audio` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum AudioChunkTag {
    /// Transport discriminator.
    #[serde(rename = "input_audio")]
    Value,
}

/// Message content is text or a list of typed chunks.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum MessageContent {
    /// Plain text.
    Text(String),
    /// Typed content chunks.
    Chunks(Vec<Content>),
}

/// System content permits only text and thinking chunks.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum SystemMessageContent {
    /// Plain instruction text.
    Text(String),
    /// Typed instruction chunks.
    Chunks(Vec<SystemContent>),
}

/// Typed content alternatives with their literal discriminator.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum ContentCandidate {
    /// Image input.
    Image(Object<ImageURLChunk>),
    /// Document input.
    Document(Object<DocumentURLChunk>),
    /// Text input.
    Text(Object<TextChunk>),
    /// Reference input.
    Reference(Object<ReferenceChunk>),
    /// File input.
    File(Object<FileChunk>),
    /// Thinking content.
    Thinking(Object<ThinkChunk>),
    /// Audio input.
    Audio(Object<AudioChunk>),
}

/// A content chunk requires an explicit discriminator.
#[derive(Serialize)]
#[serde(transparent)]
pub(super) struct Content(ContentCandidate);

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let candidate = ContentCandidate::deserialize(deserializer)?;
        let present = match &candidate {
            ContentCandidate::Image(value) => value.0.kind.is_some(),
            ContentCandidate::Document(value) => value.0.kind.is_some(),
            ContentCandidate::Text(value) => value.0.kind.is_some(),
            ContentCandidate::Reference(value) => value.0.kind.is_some(),
            ContentCandidate::File(value) => value.0.kind.is_some(),
            ContentCandidate::Thinking(value) => value.0.kind.is_some(),
            ContentCandidate::Audio(value) => value.0.kind.is_some(),
        };
        if !present {
            return Err(D::Error::missing_field("type"));
        }
        Ok(Self(candidate))
    }
}

/// Instruction chunks select only the two admitted content types.
#[derive(Serialize, Deserialize)]
#[serde(untagged, try_from = "Content")]
pub(super) enum SystemContent {
    /// Text instruction.
    Text(Object<TextChunk>),
    /// Thinking instruction.
    Thinking(Object<ThinkChunk>),
}

impl TryFrom<Content> for SystemContent {
    type Error = &'static str;
    fn try_from(value: Content) -> Result<Self, Self::Error> {
        match value.0 {
            ContentCandidate::Text(value) => Ok(Self::Text(value)),
            ContentCandidate::Thinking(value) => Ok(Self::Thinking(value)),
            _ => Err("expected text or thinking system content"),
        }
    }
}

/// Image locations accept a structured URL or a string.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum ImageLocation {
    /// Structured URL.
    Object(Object<ImageURL>),
    /// Authored URL.
    String(String),
}

/// A reference identifier is a string or safe integer.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum ReferenceId {
    /// String identity.
    String(String),
    /// Numeric identity.
    Integer(SafeInteger),
}

/// The richest surviving thinking shape, with source tie order.
#[derive(Serialize)]
#[serde(untagged)]
enum ThinkingPart {
    /// Tool reference details.
    Tool(Object<ToolReferenceChunk>),
    /// Text details.
    Text(Object<TextChunk>),
    /// Reference identities.
    Reference(Object<ReferenceChunk>),
}

impl<'de> Deserialize<'de> for ThinkingPart {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        let tool = Object::<ToolReferenceChunk>::deserialize(&value).ok();
        let text = Object::<TextChunk>::deserialize(&value).ok();
        let reference = Object::<ReferenceChunk>::deserialize(&value).ok();
        let mut candidates = Vec::with_capacity(3);
        if let Some(mut tool) = tool {
            tool.0.kind.get_or_insert_with(Default::default);
            let count = 3
                + usize::from(tool.0.url.is_some())
                + usize::from(tool.0.favicon.is_some())
                + usize::from(tool.0.description.is_some());
            candidates.push((count, Self::Tool(tool)));
        }
        if let Some(mut text) = text {
            text.0.kind.get_or_insert_with(Default::default);
            candidates.push((2, Self::Text(text)));
        }
        if let Some(mut reference) = reference {
            reference.0.kind.get_or_insert_with(Default::default);
            candidates.push((
                1 + reference.0.reference_ids.len(),
                Self::Reference(reference),
            ));
        }
        candidates
            .into_iter()
            .rev()
            .max_by_key(|(count, _)| *count)
            .map(|(_, candidate)| candidate)
            .ok_or_else(|| D::Error::custom("expected a thinking part"))
    }
}
