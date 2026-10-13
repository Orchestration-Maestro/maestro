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

/// Blob bytes. The payload hook sees them as a number array; the body carries them as text.
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

/// The selected members of `ImageSource`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ImageSource {
    /// The `bytes` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bytes: Option<Blob>,
    /// The `s3Location` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) s3_location: Option<S3Location>,
}

/// The selected members of `ImageBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ImageBlock {
    /// The `format` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) format: Option<String>,
    /// The `source` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<ImageSource>,
    /// The `error` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<ErrorBlock>,
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

/// The selected members of `ReasoningContentBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ReasoningContentBlock {
    /// The `reasoningText` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning_text: Option<ReasoningTextBlock>,
    /// The `redactedContent` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) redacted_content: Option<Blob>,
}

/// The selected members of `ToolUseBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolUseBlock {
    /// The `toolUseId` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_use_id: Option<String>,
    /// The `name` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    /// The `input` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input: Option<Value>,
    /// The `type` member.
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub(super) kind: Option<String>,
}

/// The selected members of `ToolResultContentBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolResultContentBlock {
    /// The `json` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) json: Option<Value>,
    /// The `text` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// The `image` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) image: Option<ImageBlock>,
    /// The `document` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) document: Option<DocumentBlock>,
    /// The `video` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) video: Option<VideoBlock>,
    /// The `searchResult` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) search_result: Option<SearchResultBlock>,
}

/// The selected members of `ToolResultBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolResultBlock {
    /// The `toolUseId` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_use_id: Option<String>,
    /// The `content` member.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) content: Option<Vec<ToolResultContentBlock>>,
    /// The `status` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) status: Option<String>,
    /// The `type` member.
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub(super) kind: Option<String>,
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

/// The selected members of `ContentBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ContentBlock {
    /// The `text` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// The `image` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) image: Option<ImageBlock>,
    /// The `document` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) document: Option<DocumentBlock>,
    /// The `video` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) video: Option<VideoBlock>,
    /// The `audio` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) audio: Option<AudioBlock>,
    /// The `toolUse` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_use: Option<ToolUseBlock>,
    /// The `toolResult` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_result: Option<ToolResultBlock>,
    /// The `guardContent` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) guard_content: Option<GuardrailConverseContentBlock>,
    /// The `cachePoint` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cache_point: Option<CachePointBlock>,
    /// The `reasoningContent` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning_content: Option<ReasoningContentBlock>,
    /// The `citationsContent` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) citations_content: Option<CitationsContentBlock>,
    /// The `searchResult` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) search_result: Option<SearchResultBlock>,
}

/// The selected members of `SystemContentBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct SystemContentBlock {
    /// The `text` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// The `guardContent` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) guard_content: Option<GuardrailConverseContentBlock>,
    /// The `cachePoint` member.
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

/// The selected members of `ToolSpecification`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ToolSpecification {
    /// The `name` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    /// The `inputSchema` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input_schema: Option<ToolInputSchema>,
    /// The `description` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) description: Option<String>,
    /// The `strict` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) strict: Option<bool>,
}

/// The selected members of `Tool`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Tool {
    /// The `toolSpec` member.
    #[serde(rename = "toolSpec", skip_serializing_if = "Option::is_none")]
    pub(super) spec: Option<ToolSpecification>,
    /// The `systemTool` member.
    #[serde(rename = "systemTool", skip_serializing_if = "Option::is_none")]
    pub(super) system: Option<SystemTool>,
    /// The `cachePoint` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cache_point: Option<CachePointBlock>,
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

/// The selected members of `S3Location`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct S3Location {
    /// The `uri` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) uri: Option<String>,
    /// The `bucketOwner` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bucket_owner: Option<String>,
}

/// The selected members of `ErrorBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ErrorBlock {
    /// The `message` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) message: Option<String>,
}

/// The selected members of `TextBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct TextBlock {
    /// The `text` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
}

/// The selected members of `DocumentBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct DocumentBlock {
    /// The `name` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    /// The `source` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<DocumentSource>,
    /// The `format` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) format: Option<String>,
    /// The `context` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) context: Option<String>,
    /// The `citations` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) citations: Option<CitationsConfig>,
}

/// The selected members of `DocumentSource`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct DocumentSource {
    /// The `bytes` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bytes: Option<Blob>,
    /// The `s3Location` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) s3_location: Option<S3Location>,
    /// The `text` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// The `content` member.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) content: Option<Vec<DocumentContentBlock>>,
}

/// Known fields of `DocumentContentBlock`.
pub(super) type DocumentContentBlock = TextBlock;

/// The selected members of `CitationsConfig`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct CitationsConfig {
    /// The `enabled` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
}

/// The selected members of `VideoBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct VideoBlock {
    /// The `format` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) format: Option<String>,
    /// The `source` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<VideoSource>,
}

/// Known fields of `VideoSource`.
pub(super) type VideoSource = ImageSource;

/// The selected members of `AudioBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct AudioBlock {
    /// The `format` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) format: Option<String>,
    /// The `source` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<AudioSource>,
    /// The `error` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<ErrorBlock>,
}

/// Known fields of `AudioSource`.
pub(super) type AudioSource = ImageSource;

/// The selected members of `SearchResultBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct SearchResultBlock {
    /// The `source` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
    /// The `title` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) title: Option<String>,
    /// The `content` member.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) content: Option<Vec<SearchResultContentBlock>>,
    /// The `citations` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) citations: Option<CitationsConfig>,
}

/// Known fields of `SearchResultContentBlock`.
pub(super) type SearchResultContentBlock = TextBlock;

/// The selected members of `GuardrailConverseContentBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct GuardrailConverseContentBlock {
    /// The `text` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<GuardrailConverseTextBlock>,
    /// The `image` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) image: Option<GuardrailConverseImageBlock>,
}

/// The selected members of `GuardrailConverseTextBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct GuardrailConverseTextBlock {
    /// The `text` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    /// The `qualifiers` member.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) qualifiers: Option<Vec<String>>,
}

/// The selected members of `GuardrailConverseImageBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct GuardrailConverseImageBlock {
    /// The `format` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) format: Option<String>,
    /// The `source` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<GuardrailConverseImageSource>,
}

/// The selected members of `GuardrailConverseImageSource`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct GuardrailConverseImageSource {
    /// The `bytes` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bytes: Option<Blob>,
}

/// The selected members of `CitationsContentBlock`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct CitationsContentBlock {
    /// The `content` member.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) content: Option<Vec<CitationGeneratedContent>>,
    /// The `citations` member.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) citations: Option<Vec<Citation>>,
}

/// Known fields of `CitationGeneratedContent`.
pub(super) type CitationGeneratedContent = TextBlock;

/// The selected members of `Citation`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Citation {
    /// The `title` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) title: Option<String>,
    /// The `source` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
    /// The `sourceContent` member.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "list")]
    pub(super) source_content: Option<Vec<CitationSourceContent>>,
    /// The `location` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) location: Option<CitationLocation>,
}

/// Known fields of `CitationSourceContent`.
pub(super) type CitationSourceContent = TextBlock;

/// The selected members of `CitationLocation`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct CitationLocation {
    /// The `web` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) web: Option<WebLocation>,
    /// The `documentChar` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) document_char: Option<DocumentCharLocation>,
    /// The `documentPage` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) document_page: Option<DocumentPageLocation>,
    /// The `documentChunk` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) document_chunk: Option<DocumentChunkLocation>,
    /// The `searchResultLocation` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) search_result_location: Option<SearchResultLocation>,
}

/// The selected members of `WebLocation`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct WebLocation {
    /// The `url` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) url: Option<String>,
    /// The `domain` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) domain: Option<String>,
}

/// The selected members of `DocumentCharLocation`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct DocumentCharLocation {
    /// The `documentIndex` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) document_index: Option<f64>,
    /// The `start` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) start: Option<f64>,
    /// The `end` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) end: Option<f64>,
}

/// Known fields of `DocumentPageLocation`.
pub(super) type DocumentPageLocation = DocumentCharLocation;

/// Known fields of `DocumentChunkLocation`.
pub(super) type DocumentChunkLocation = DocumentCharLocation;

/// The selected members of `SearchResultLocation`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct SearchResultLocation {
    /// The `searchResultIndex` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) search_result_index: Option<f64>,
    /// The `start` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) start: Option<f64>,
    /// The `end` member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) end: Option<f64>,
}

/// Known fields of `SystemTool`.
pub(super) type SystemTool = NamedToolChoice;
