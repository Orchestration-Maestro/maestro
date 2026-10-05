//! Owned requests, response records and secret-safe outcomes.

use crate::AssistantContent;

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
#[derive(Clone, PartialEq, Eq)]
pub struct Model {
    /// Complete dispatch identity.
    pub identity: ModelIdentity,
    /// Adapter protocol identifier.
    pub protocol: String,
    /// Registered literal default headers; sensitive and not safe to log.
    pub headers: std::collections::BTreeMap<String, String>,
    /// Supplied input capabilities; `image` enables image retention.
    pub input: Vec<String>,
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

/// The five terminal outcomes of a chat response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// Successful termination.
    Stop,
    /// Output limit reached.
    Length,
    /// Successful tool-call response.
    ToolUse,
    /// Failed termination.
    Error,
    /// Local cancellation.
    Aborted,
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
    /// Invalid block updates or completed tool JSON.
    MalformedStream,
    /// Transport lost during a request.
    Transport,
    /// Local request work was cancelled.
    Cancelled,
    /// No usable request authentication was supplied.
    MissingAuthentication,
    /// Request credential resolution or exchange failed.
    AuthenticationFailed,
    /// Malformed or ambiguous literal header input.
    InvalidRequestHeaders,
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
            Self::MalformedStream => "malformed provider stream",
            Self::Transport => "provider transport failed",
            Self::Cancelled => "request cancelled",
            Self::MissingAuthentication => "missing request authentication",
            Self::AuthenticationFailed => "request authentication failed",
            Self::InvalidRequestHeaders => "invalid request headers",
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
    /// Cumulative independent content blocks.
    pub content: Vec<AssistantContent>,
    /// Reported usage; zero counters initially mean unreported usage.
    pub usage: Usage,
    /// Absent in partial snapshots; present in terminal records.
    pub stop_reason: Option<StopReason>,
    /// Typed failure category on failed termination.
    pub failure: Option<Failure>,
    /// Actual response model, distinct from requested identity.
    pub response_model: Option<String>,
    /// Actual response identifier.
    pub response_id: Option<String>,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("identity", &self.identity)
            .field("protocol", &self.protocol)
            .field("headers", &crate::provider::RedactedHeaders(&self.headers))
            .finish()
    }
}
