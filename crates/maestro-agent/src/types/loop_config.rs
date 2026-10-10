//! Conversation inputs and model-boundary callbacks.
use super::{AgentMessage, CustomAgentMessages, SharedAgentTool};
use maestro_models::{
    AssistantMessageEventStream, BoxFuture, Cancellation, Context, DiagnosticErrorInfo, JsonObject,
    Message, Model, ProviderObjects, ProviderStreamOptions, SimpleStreamOptions,
};
use std::{
    convert::Infallible,
    sync::{Arc, RwLock},
};

/// Retained ordered conversation entries.
pub(crate) type Messages<C> = Arc<RwLock<Vec<AgentMessage<C>>>>;
/// Instructions and retained conversation inputs for one operation.
pub struct AgentContext<C: CustomAgentMessages = Infallible> {
    /// Model instructions.
    pub system_prompt: String,
    /// Mutable ordered history.
    pub messages: Arc<RwLock<Vec<AgentMessage<C>>>>,
    /// Optional mutable executable-tool inventory.
    pub tools: Option<Arc<RwLock<Vec<SharedAgentTool>>>>,
}
impl<C: CustomAgentMessages> Clone for AgentContext<C> {
    fn clone(&self) -> Self {
        Self {
            system_prompt: self.system_prompt.clone(),
            messages: Arc::clone(&self.messages),
            tools: self.tools.clone(),
        }
    }
}
/// Model selection and callbacks for each request.
pub struct AgentLoopConfig<C: CustomAgentMessages = Infallible> {
    /// Supplied model descriptor.
    pub model: Model,
    /// Simple invocation settings.
    pub options: SimpleStreamOptions,
    /// Open settings passed to a replacement stream.
    pub extra: JsonObject,
    /// Typed objects passed to a replacement stream.
    pub objects: ProviderObjects,
    /// Convert conversation entries at the request boundary.
    pub convert_to_llm: ConvertToLlm<C>,
    /// Optional transformation before conversion.
    pub transform_context: TransformContext<C>,
    /// Optional per-request credential resolver.
    pub get_api_key: GetApiKey,
}
/// Convert shared conversation entries into model messages.
#[cfg(not(target_arch = "wasm32"))]
pub type ConvertToLlm<C> =
    Arc<dyn Fn(Messages<C>) -> BoxFuture<Result<Vec<Message>, DiagnosticErrorInfo>> + Send + Sync>;
/// Convert browser-local conversation entries into model messages.
#[cfg(target_arch = "wasm32")]
pub type ConvertToLlm<C> =
    Arc<dyn Fn(Messages<C>) -> BoxFuture<Result<Vec<Message>, DiagnosticErrorInfo>>>;
/// Optionally transform retained history before conversion.
#[cfg(not(target_arch = "wasm32"))]
pub type TransformContext<C> = Option<
    Arc<
        dyn Fn(
                Messages<C>,
                Option<Cancellation>,
            ) -> BoxFuture<Result<Messages<C>, DiagnosticErrorInfo>>
            + Send
            + Sync,
    >,
>;
/// Optionally transform browser-local history before conversion.
#[cfg(target_arch = "wasm32")]
pub type TransformContext<C> = Option<
    Arc<
        dyn Fn(
            Messages<C>,
            Option<Cancellation>,
        ) -> BoxFuture<Result<Messages<C>, DiagnosticErrorInfo>>,
    >,
>;
/// Optionally resolve a provider credential for each request.
#[cfg(not(target_arch = "wasm32"))]
pub type GetApiKey = Option<
    Arc<dyn Fn(String) -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>> + Send + Sync>,
>;
/// Optionally resolve a provider credential in the browser.
#[cfg(target_arch = "wasm32")]
pub type GetApiKey =
    Option<Arc<dyn Fn(String) -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>>>>;
/// Construct a replacement model stream with simple and open options.
#[cfg(not(target_arch = "wasm32"))]
pub type StreamFn = Arc<
    dyn Fn(
            Model,
            Context,
            SimpleStreamOptions,
            ProviderStreamOptions,
        ) -> BoxFuture<Result<AssistantMessageEventStream, DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
/// Construct a browser-local replacement model stream.
#[cfg(target_arch = "wasm32")]
pub type StreamFn = Arc<
    dyn Fn(
        Model,
        Context,
        SimpleStreamOptions,
        ProviderStreamOptions,
    ) -> BoxFuture<Result<AssistantMessageEventStream, DiagnosticErrorInfo>>,
>;
