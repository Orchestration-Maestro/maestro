//! Runtime-independent model access through explicitly registered adapters.
//! Conversation projection supplies independent owned request views without
//! storing history or executing tools; pure validation returns owned arguments.
//! Indexed text, thinking and tool-call updates become independent owned
//! snapshots with strict block validation and exactly one terminal outcome.
//! Flat reported counters yield checked totals and catalog-rate estimates, not bills.
//! Reporting and supplied-price provenance remain independent on partial failures.
//! Completion drains the same stream. Supplied cancellation wakes blocked reads
//! and releases local work without claiming to undo remote effects.
//! Selected-provider request authentication is supplied explicitly or resolved
//! once; credential lifecycle remains outside this crate.

#![doc = include_str!("../../../docs/models.md")]
#![doc = include_str!("../../../docs/model-options.md")]
#![doc = include_str!("../../../docs/local-model-catalogs.md")]
#![doc = include_str!("../../../docs/request-authentication.md")]
#![doc = include_str!("../../../docs/conversation-projection.md")]
#![doc = include_str!("../../../docs/chat-connection.md")]

mod accounting;
mod auth;
mod cancellation;
mod catalog;
mod content;
mod conversation;
mod dispatch;
mod events;
mod options;
mod projection;
mod provider;
mod registry;
mod scalar;
mod schema;
mod scripted;
mod stream;
mod types;
mod validation;

pub use accounting::{TokenRates, Usage, UsageCost};
pub use auth::{
    AuthResolver, AuthStatus, RequestAuth, SecretString, TokenExchange, TokenExchangeResult,
};
pub use cancellation::Cancellation;
pub use catalog::{AvailableModel, CatalogOverride, ChatMetadata};
pub use content::{
    AssistantContent, ImageContent, InputContent, TextContent, ThinkingContent, ToolCall,
};
pub use conversation::{Context, Message, ToolDeclaration, ToolResultMessage, UserMessage};
pub use events::ModelEvent;
pub use options::{
    EffectiveOptions, RequestCapabilities, StreamOptions, ThinkingLevel, ThinkingMode, ToolChoice,
};
pub use projection::project_context;
pub use provider::{
    Provider, ProviderDescription, ProviderOptions, ProviderStream, ProviderUpdate,
};
pub use registry::Models;
pub use scripted::{Script, ScriptFactory, ScriptStep, ScriptedCall, ScriptedProvider};
pub use stream::ModelStream;
pub use types::{AssistantMessage, Failure, Model, ModelIdentity, StopReason};
pub use validation::{ToolValidationError, validate_tool_call};

mod chat;
mod chat_dialect;
mod chat_transport;
pub use chat::ChatConnection;
pub use chat_dialect::{ChatCacheControl, ChatDialect, ChatOutputField, ChatThinkingFormat};
pub use chat_transport::{ChatHttpBody, ChatHttpRequest, ChatHttpResponse, ChatTransport};

mod chat_request;

mod chat_replay;

mod chat_json;

mod chat_decode;

mod chat_sse;

mod chat_failure;

mod chat_retry;

mod http_transport;
pub use http_transport::NativeHttpTransport;
