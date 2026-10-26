//! Shared invocation options specialized with retained local guest capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{AbortSignal, ExtensionFuture, Model};
use std::{any::Any, convert::Infallible, rc::Rc, sync::Arc};

/// Requested reasoning excludes the disabled level admitted by agent selection.
pub use maestro_request::types::ThinkingLevel as RequestedThinkingLevel;
pub use maestro_request::types::{
    CacheRetention, ProviderResponse, StreamOptionSlots, ThinkingBudgets, Transport,
};

/// Selected provider payload and its independently selectable cancellation.
pub struct PayloadHookInput {
    /// Owned request parameters, including scalar or null replacements.
    pub payload: serde_json::Value,
    /// Retained cancellation; absence removes the previous signal.
    pub signal: Option<AbortSignal>,
}

/// Awaited selector of the payload and cancellation supplied to the next consumer.
pub type OnPayload =
    Rc<dyn Fn(PayloadHookInput, Arc<Model>) -> ExtensionFuture<'static, PayloadHookInput>>;
/// Awaited observer called before consuming a streaming response body.
pub type OnResponse = Rc<dyn Fn(ProviderResponse, Arc<Model>) -> ExtensionFuture<'static, ()>>;
/// Retained opaque author controls; no JSON conversion or native-client interpretation.
pub type ProviderObjects = Option<Rc<dyn Any>>;

/// Guest-local capability representations for shared invocation options.
#[derive(Clone)]
pub struct Local;
impl StreamOptionSlots for Local {
    type Signal = AbortSignal;
    type PayloadHook = OnPayload;
    type ResponseHook = OnResponse;
    type Fetch = Infallible;
}

/// Common invocation data with local hooks and cancellation, without a fetch operation.
pub type StreamOptions = maestro_request::types::StreamOptions<Local>;
/// Open provider options with retained opaque author controls.
pub type ProviderStreamOptions =
    maestro_request::types::ProviderStreamOptions<StreamOptions, ProviderObjects>;
/// Simple invocation data without an authored tool-choice operation.
pub type SimpleStreamOptions =
    maestro_request::types::SimpleStreamOptions<StreamOptions, Infallible>;
/// Original raw or simple callback options with guest-local capability representations.
pub type StreamOptionsInput =
    maestro_request::types::StreamOptionsInput<StreamOptions, ProviderObjects, Infallible>;

/// Retained local provider invoked with the original option variant.
pub type StreamFunction<O = ProviderStreamOptions> = Rc<
    dyn Fn(
        Model,
        crate::Context,
        Option<O>,
    ) -> crate::ExtensionResult<crate::AssistantMessageEventStream>,
>;
