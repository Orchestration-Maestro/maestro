//! Owned text requests, response snapshots and typed outcomes.

/// The complete dispatch key; all three identifiers are opaque data.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelIdentity {
    /// Provider identifier.
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// Operation identifier.
    pub operation: String,
}

/// Explicit model reference supplied at registration and invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// Complete dispatch identity.
    pub identity: ModelIdentity,
    /// Adapter protocol identifier.
    pub protocol: String,
}

/// One text user input with its Unix timestamp in milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserMessage {
    /// User text.
    pub content: String,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Current system prompt and ordered text user inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    /// Separate optional system prompt.
    pub system_prompt: Option<String>,
    /// User inputs in conversation order.
    pub messages: Vec<UserMessage>,
}

/// One cumulative assistant text block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextContent {
    /// Accumulated text.
    pub text: String,
}

/// Reported flat token counters, not estimates inferred from text. Initial zeros mean unreported usage.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Usage {
    /// Reported input tokens.
    pub input: u64,
    /// Reported output tokens.
    pub output: u64,
    /// Reported cache-read tokens.
    pub cache_read: u64,
    /// Reported cache-write tokens.
    pub cache_write: u64,
    /// Reported total tokens.
    pub total_tokens: u64,
}

/// The successful or failed terminal outcomes exercised by text access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// Successful termination.
    Stop,
    /// Failed termination.
    Error,
}

/// Recoverable categories whose display text is fixed and never includes request or adapter secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// No registration has the requested provider identifier.
    UnknownProvider,
    /// The provider has no registration for the requested model identity.
    UnknownModel,
    /// The operation or protocol is unsupported.
    UnsupportedOperation,
    /// An exact model identity is already registered.
    DuplicateModel,
    /// No scripted response remains.
    ScriptExhausted,
    /// The update source ended without a terminal update.
    IncompleteStream,
    /// The adapter could not set up or continue the request.
    AdapterFailed,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownProvider => "unknown provider: register a provider before invoking it",
            Self::UnknownModel => "unknown model: register the requested model identity",
            Self::UnsupportedOperation => "unsupported operation or protocol",
            Self::DuplicateModel => "model identity is already registered",
            Self::ScriptExhausted => "scripted provider has no response remaining",
            Self::IncompleteStream => "provider stream ended without a terminal update",
            Self::AdapterFailed => "provider adapter failed",
        })
    }
}

impl std::error::Error for Failure {}

/// An independent owned response-so-far, or the final successful/failed assistant record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantMessage {
    /// Requested provider identifier.
    pub provider: String,
    /// Requested protocol identifier.
    pub protocol: String,
    /// Requested model identifier.
    pub model: String,
    /// Unix-millisecond clock sampled once at invocation.
    pub timestamp: u64,
    /// Cumulative independent text blocks.
    pub content: Vec<TextContent>,
    /// Reported usage; zero counters initially mean unreported usage.
    pub usage: Usage,
    /// Absent in partial snapshots; present in terminal records.
    pub stop_reason: Option<StopReason>,
    /// Typed failure category on failed termination.
    pub failure: Option<Failure>,
}

/// Typed stream events carrying independent, cumulative owned snapshots.
/// Successful text follows start, text-start, deltas, text-end, done ordering.
/// Errors terminate without a done event; empty success has no text block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelEvent {
    /// The first successful update was obtained, with no accumulated content yet.
    Start {
        /// Independent response-so-far.
        partial: AssistantMessage,
    },
    /// The text block was opened before its first delta.
    TextStart {
        /// Index of the text block, currently zero.
        content_index: usize,
        /// Snapshot containing the empty block.
        partial: AssistantMessage,
    },
    /// A supplied chunk was appended.
    TextDelta {
        /// Index of the text block, currently zero.
        content_index: usize,
        /// The supplied chunk.
        delta: String,
        /// Snapshot after appending the chunk.
        partial: AssistantMessage,
    },
    /// The text block closed successfully with final reported usage.
    TextEnd {
        /// Index of the text block, currently zero.
        content_index: usize,
        /// Full accumulated text.
        content: String,
        /// Independent response-so-far, not yet terminal.
        partial: AssistantMessage,
    },
    /// Successful terminal delivery.
    Done {
        /// Always [`StopReason::Stop`].
        reason: StopReason,
        /// Final successful record.
        message: AssistantMessage,
    },
    /// Failed terminal delivery, possibly retaining partial text.
    Error {
        /// Always [`StopReason::Error`].
        reason: StopReason,
        /// Final failed record, with a fixed-display failure category.
        error: AssistantMessage,
    },
}
