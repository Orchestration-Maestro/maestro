//! Existing models owners exposed at the application root.

pub use maestro_models::{ModelCost, Usage, UsageCost};

pub use maestro_agent::{AgentMessage, CustomAgentMessages, NoCustomMessages, StreamFn};

pub use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEventStream, BoxFuture, Cancellation,
    Context, DiagnosticErrorInfo, Fetch, FetchError, HttpBody, HttpRequest, HttpResponse,
    ImageContent, JsonObject, Message, Model, ModelInput, OAuthCallbacks, OAuthCredentials,
    OAuthError, OAuthProviderInterface, ProviderStreamOptions, SharedAssistantMessage,
    SharedHandle, SimpleStreamOptions, StopReason, TextContent, ToolCall, ToolResultMessage,
    UserBlock, UserContent, UserMessage, default_fetch,
};

/// Agent reasoning levels, including disabled reasoning, and how queued messages are delivered.
pub use maestro_agent::{QueueMode, ThinkingLevel};

pub use maestro_models::{Local, Threaded};

pub use maestro_models::Transport;

pub use maestro_catalog::{
    CatalogError, ConfigValues, ModelFileOperations, ModelRegistry, ModelResolverOutput,
    ProviderConfigInput, ProviderModelInput, ScopedModel, find_exact_model_reference_match,
    model_id_comparator, resolve_model_scope,
};

pub use maestro_models::models_are_equal;

/// Provider identifier resolved when no model is chosen, shared with the catalog selector.
pub use maestro_catalog::DEFAULT_MODEL_PER_PROVIDER;

/// Shipped provider labels, shared with registry display-name lookups.
pub use maestro_catalog::BUILT_IN_PROVIDER_DISPLAY_NAMES;

/// Provider identifiers in embedded catalog order.
pub use maestro_models::get_providers;

/// Authorization choice prompt retaining supplied option order.
pub use maestro_models::OAuthSelectPrompt;

/// One supplied authorization choice.
pub use maestro_models::OAuthSelectOption;

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_models::default_web_socket;

pub use maestro_models::{
    ConnectWebSocket, WebSocket, WebSocketConnection, WebSocketFuture, WebSocketRequest,
};
