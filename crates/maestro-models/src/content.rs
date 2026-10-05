//! Owned input and assistant content with strictly completed tool arguments.

/// One cumulative text block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextContent {
    /// Accumulated readable text.
    pub text: String,
    /// Opaque replay metadata, independent of readable text.
    pub replay_metadata: Option<String>,
}

/// Readable reasoning or opaque redacted thinking; the alternatives never mix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThinkingContent {
    /// Reasoning text with optional opaque replay information.
    Readable {
        /// Accumulated readable reasoning.
        text: String,
        /// Optional opaque replay signature.
        signature: Option<String>,
    },
    /// Opaque data that must not be presented as readable reasoning.
    Redacted {
        /// Unmodified opaque replay data.
        data: String,
    },
}

/// An identified tool call with arguments unavailable until a valid object ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCall {
    /// Explicit tool-call identifier.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Optional opaque replay metadata.
    pub replay_metadata: Option<String>,
    pub(crate) arguments: Option<serde_json::Map<String, serde_json::Value>>,
}
impl ToolCall {
    /// Construct completed object arguments without validating or executing them.
    pub fn new(
        id: String,
        name: String,
        arguments: serde_json::Map<String, serde_json::Value>,
        replay_metadata: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            arguments: Some(arguments),
            replay_metadata,
        }
    }

    /// Return completed object arguments, unavailable before a valid tool-call end.
    /// This is not execution authorization: require successful terminal completion
    /// and apply caller-owned tool validation and policy before executing.
    pub fn arguments(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.arguments.as_ref()
    }
}

/// One indexed assistant response block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantContent {
    /// Readable answer text.
    Text(TextContent),
    /// Readable reasoning or opaque redacted thinking.
    Thinking(ThinkingContent),
    /// An identified tool call with arguments unavailable until a valid object ends.
    ToolCall(ToolCall),
}

/// Supplied base64 image data, never decoded or preprocessed by model access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageContent {
    /// Exact supplied base64 string.
    pub data: String,
    /// Exact supplied MIME type.
    pub mime_type: String,
}

/// One ordered input block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputContent {
    /// Readable text with optional opaque replay data.
    Text(TextContent),
    /// Supplied image bytes represented as base64.
    Image(ImageContent),
}
