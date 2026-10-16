#![doc = include_str!("../../../../../../docs/models/response-sessions.md")]

#[cfg(not(target_arch = "wasm32"))]
mod continuation;
mod debug;
mod events;
mod headers;
mod host;
mod http;
mod request;
mod stream;
pub(crate) use stream::{dispatch_simple, select};
pub use stream::{
    stream_openai_codex_responses, stream_openai_codex_responses_for,
    stream_simple_openai_codex_responses, stream_simple_openai_codex_responses_for,
};
#[cfg(not(target_arch = "wasm32"))]
mod sessions;
#[cfg(not(target_arch = "wasm32"))]
mod websocket;

#[cfg(test)]
mod tests;

pub use crate::providers::nullable::Nullable;
use crate::providers::responses::openai_responses::OpenAIResponsesServiceTier;
use crate::{ProviderExecution, StreamOptions, Threaded};
use serde::{Deserialize, Serialize};

/// Raw invocation settings.
#[derive(Clone, Default)]
pub struct OpenAICodexResponsesOptions<E: ProviderExecution = Threaded> {
    /// Shared hooks and transport selections.
    pub common: StreamOptions<E>,
    /// Omitted, explicitly null, or named service tier.
    pub service_tier: Option<Nullable<OpenAIResponsesServiceTier>>,
    /// Explicit raw effort, independent of descriptor capability.
    pub reasoning_effort: Option<crate::ModelThinkingLevel>,
    /// Optional summary; omission and null select auto when effort is requested.
    pub reasoning_summary: Option<OpenAICodexReasoningSummary>,
    /// Requested output verbosity.
    pub text_verbosity: Option<OpenAICodexTextVerbosity>,
}

/// Requested text detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenAICodexTextVerbosity {
    /// Brief text.
    Low,
    /// Intermediate detail.
    Medium,
    /// Extensive detail.
    High,
}

/// Requested reasoning summary style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenAICodexReasoningSummary {
    /// Provider-selected summary.
    Auto,
    /// Brief summary.
    Concise,
    /// Detailed summary.
    Detailed,
    /// Disable summary.
    Off,
    /// Enable summary.
    On,
}

/// Source failures retain their category independently of their rendered text.
#[derive(Debug)]
pub(crate) enum CodexError {
    /// Server-declared API failure.
    Api(crate::DiagnosticErrorInfo),
    /// Malformed provider data or shared reduction failure.
    Protocol(crate::DiagnosticErrorInfo),
    /// HTTP transport/setup failure.
    Transport(crate::DiagnosticErrorInfo),
}
impl CodexError {
    /// Borrow the original diagnostic without changing its identity.
    fn diagnostic(&self) -> &crate::DiagnosticErrorInfo {
        match self {
            Self::Api(error) | Self::Protocol(error) | Self::Transport(error) => error,
        }
    }
}

impl From<crate::DiagnosticErrorInfo> for CodexError {
    fn from(error: crate::DiagnosticErrorInfo) -> Self {
        Self::Transport(error)
    }
}

impl From<crate::FetchError> for CodexError {
    fn from(error: crate::FetchError) -> Self {
        match error {
            crate::FetchError::Connection(error) => Self::Transport(error),
            crate::FetchError::Aborted => {
                let mut error = request::diagnostic("Request was aborted");
                error.name = Some("AbortError".to_owned());
                Self::Transport(error)
            }
            crate::FetchError::Timeout => {
                Self::Transport(request::diagnostic("Request timed out."))
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use sessions::close_openai_codex_web_socket_sessions;

/// Browsers retain no authenticated response-session socket.
#[cfg(target_arch = "wasm32")]
pub fn close_openai_codex_web_socket_sessions(_session_id: Option<&str>) {}

pub use debug::{
    OpenAICodexWebSocketDebugStats, get_openai_codex_web_socket_debug_stats,
    reset_openai_codex_web_socket_debug_stats,
};
