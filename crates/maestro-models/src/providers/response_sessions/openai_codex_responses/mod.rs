//! Internal response-session request preparation.

mod headers;
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
