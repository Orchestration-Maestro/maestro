//! Existing models owners exposed at the application root.

pub use maestro_models::{ModelCost, Usage, UsageCost};

pub use maestro_agent::{AgentMessage, CustomAgentMessages, StreamFn};

pub use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEventStream, BoxFuture, Cancellation,
    Context, DiagnosticErrorInfo, Fetch, FetchError, HttpBody, HttpRequest, HttpResponse,
    ImageContent, Message, Model, ModelInput, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface, ProviderStreamOptions, SharedAssistantMessage, SharedHandle,
    SimpleStreamOptions, StopReason, TextContent, ToolResultMessage, UserBlock, UserContent,
    default_fetch,
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
