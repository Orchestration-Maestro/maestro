#![doc = include_str!("../../../../../../docs/models/responses.md")]

mod events;
pub(crate) mod messages;
mod wire;

use futures_util::StreamExt;

use crate::{
    AssistantMessageEventStream, DiagnosticErrorInfo, Model, SharedAssistantMessage, Usage,
};
use events::Reducer;

/// Options for [`messages::convert_responses_messages`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct ConvertResponsesMessagesOptions {
    /// Whether the system prompt leads the converted items; `true` by default.
    pub(crate) include_system_prompt: bool,
}

impl Default for ConvertResponsesMessagesOptions {
    fn default() -> Self {
        Self {
            include_system_prompt: true,
        }
    }
}

/// Options for [`messages::convert_responses_tools`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct ConvertResponsesToolsOptions {
    /// The strict setting of every tool: `Some(false)` by default, and `None` sends `null`.
    pub(crate) strict: Option<bool>,
}

impl Default for ConvertResponsesToolsOptions {
    fn default() -> Self {
        Self {
            strict: Some(false),
        }
    }
}

/// Callback that chooses the service tier of a response from the one it echoed and the one
/// requested.
#[cfg(not(target_arch = "wasm32"))]
type ResolveServiceTier<'a> =
    dyn Fn(Option<&str>, Option<&str>) -> Option<String> + Send + Sync + 'a;
/// Callback that chooses the service tier of a response from the one it echoed and the one
/// requested.
#[cfg(target_arch = "wasm32")]
type ResolveServiceTier<'a> = dyn Fn(Option<&str>, Option<&str>) -> Option<String> + 'a;
/// Callback that adjusts the cost of a response for its service tier.
#[cfg(not(target_arch = "wasm32"))]
type ApplyServiceTierPricing<'a> = dyn Fn(&mut Usage, Option<&str>) + Send + Sync + 'a;
/// Callback that adjusts the cost of a response for its service tier.
#[cfg(target_arch = "wasm32")]
type ApplyServiceTierPricing<'a> = dyn Fn(&mut Usage, Option<&str>) + 'a;

/// Options for [`process_responses_stream`]; every field is absent by default.
#[derive(Default)]
pub(crate) struct OpenAIResponsesStreamOptions<'a> {
    /// The service tier the request asked for.
    pub(crate) service_tier: Option<&'a str>,
    /// Chooses the tier to price from the echoed and the requested tier; it is called only
    /// when `apply_service_tier_pricing` is present.
    pub(crate) resolve_service_tier: Option<&'a ResolveServiceTier<'a>>,
    /// Adjusts the cost of a finished response for the chosen tier, after the model's own
    /// rates were applied.
    pub(crate) apply_service_tier_pricing: Option<&'a ApplyServiceTierPricing<'a>>,
}

/// A failure with the given message, as a thrown error reports it.
fn failure(message: impl Into<String>) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".to_owned()),
        message: message.into(),
        stack: None,
        code: None,
    }
}

/// A failure that keeps the message of the JSON reader that raised it.
fn native(error: &serde_json::Error) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: None,
        message: error.to_string(),
        stack: None,
        code: None,
    }
}

/// Reduce framed response events, given as JSON text, into `output`, publishing content
/// events on `stream`.
///
/// Reduction continues until `events` ends, which must follow a completed or incomplete
/// response. It publishes content updates only: it neither starts nor finishes `stream`, and
/// the caller owns the final outcome.
///
/// # Errors
/// Returns a failure the source yields unchanged, a malformed event, a failed or errored
/// response, an unknown response status, a signature nested beyond the conversion bound, or
/// `Response stream ended before a terminal event` when the source ends first. Content
/// reduced before the failure stays in `output`.
pub(crate) async fn process_responses_stream<S>(
    mut events: S,
    output: &SharedAssistantMessage,
    stream: &AssistantMessageEventStream,
    model: &Model,
    options: Option<&OpenAIResponsesStreamOptions<'_>>,
) -> Result<(), DiagnosticErrorInfo>
where
    S: futures_core::Stream<Item = Result<String, DiagnosticErrorInfo>> + Unpin,
{
    let mut reducer = Reducer::new(output, stream, model, options);
    while let Some(event) = events.next().await {
        reducer.event(&event?)?;
    }
    reducer.finish()
}

#[cfg(test)]
mod tests;
