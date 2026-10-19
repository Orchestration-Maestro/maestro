//! Existing models owners exposed at the application root.

pub use maestro_models::{ModelCost, Usage, UsageCost};

pub use maestro_agent::{
    Agent, AgentEvent, AgentListener, AgentMessage, CustomAgentMessages, NoCustomMessages,
    StreamFn, Subscription,
};

/// Construction options and prompt operands for application-owned agents.
pub use maestro_agent::{AgentOptions, PromptInput};

/// Executable tool construction and its model-facing declaration.
pub use maestro_agent::AgentToolRecord;
pub use maestro_models::Tool;

/// Execution representations required by caller-owned scripted response factories.
pub use maestro_models::{Execution, Invoke, Local, ProviderExecution, Threaded};

/// Scripted response factory input and retained callback ownership.
pub use maestro_models::{FauxResponseFactory, FauxResponseInput, StreamOptionsInput};

/// Scripted model registration and owned response construction for controlled applications.
pub use maestro_models::{
    FauxAssistantMessageOptions, FauxProviderRegistration, FauxResponseStep,
    RegisterFauxProviderOptions, faux_assistant_message, register_faux_provider,
};

pub use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEventStream, BoxFuture, Cancellation,
    Context, DiagnosticErrorInfo, Fetch, FetchError, HttpBody, HttpRequest, HttpResponse,
    ImageContent, JsonObject, Message, Model, ModelInput, OAuthCallbacks, OAuthCredentials,
    OAuthError, OAuthProviderInterface, ProviderStreamOptions, SharedAssistantMessage,
    SharedHandle, SimpleStreamOptions, StopReason, TextContent, ToolCall, ToolResultMessage,
    UserBlock, UserContent, UserMessage, default_fetch,
};

/// Agent reasoning levels, including disabled reasoning.
pub use maestro_agent::ThinkingLevel;

pub use maestro_models::Transport;

pub use maestro_catalog::{
    CatalogError, ModelFileOperations, ModelRegistry, ProviderConfigInput, ProviderModelInput,
    ScopedModel, model_id_comparator,
};

pub use maestro_models::models_are_equal;

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_models::default_web_socket;

pub use maestro_models::{
    ConnectWebSocket, WebSocket, WebSocketConnection, WebSocketFuture, WebSocketRequest,
};
