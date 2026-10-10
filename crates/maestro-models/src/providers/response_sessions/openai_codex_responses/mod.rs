//! Internal response-session request preparation.

mod events;
mod headers;
mod http;
mod request;

#[cfg(test)]
mod tests;

use crate::StreamOptions;
use crate::providers::nullable::Nullable;
use crate::providers::responses::openai_responses::OpenAIResponsesServiceTier;
use serde::Serialize;

/// Internal invocation settings.
#[derive(Clone, Default)]
pub(crate) struct OpenAICodexResponsesOptions {
    /// Shared hooks and transport selections.
    pub(crate) common: StreamOptions,
    /// Omitted, explicitly null, or named service tier.
    pub(crate) service_tier: Option<Nullable<OpenAIResponsesServiceTier>>,
    /// Explicit raw effort, independent of descriptor capability.
    pub(crate) reasoning_effort: Option<crate::ModelThinkingLevel>,
    /// Optional summary, omission and null both select auto.
    pub(crate) reasoning_summary: Option<OpenAICodexReasoningSummary>,
    /// Requested output verbosity.
    pub(crate) text_verbosity: Option<OpenAICodexTextVerbosity>,
}

/// Requested text detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAICodexTextVerbosity {
    /// Brief text.
    Low,
    /// Intermediate detail.
    Medium,
    /// Extensive detail.
    High,
}

/// Requested reasoning summary style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAICodexReasoningSummary {
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
