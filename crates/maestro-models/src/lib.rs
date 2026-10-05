//! Runtime-independent model access through explicitly registered adapters.
//! Conversation projection supplies independent owned request views without
//! storing history or executing tools; pure validation returns owned arguments.
//! Indexed text, thinking and tool-call updates become independent owned
//! snapshots with strict block validation and exactly one terminal outcome.
//! Completion drains the same stream. Supplied cancellation wakes blocked reads
//! and releases local work without claiming to undo remote effects.
//! Selected-provider request authentication is supplied explicitly or resolved
//! once; credential lifecycle remains outside this crate.

#![doc = include_str!("../../../docs/models.md")]
#![doc = include_str!("../../../docs/request-authentication.md")]
#![doc = include_str!("../../../docs/conversation-projection.md")]

mod auth;
mod cancellation;
mod content;
mod conversation;
mod dispatch;
mod events;
mod projection;
mod provider;
mod registry;
mod schema;
mod scripted;
mod stream;
mod types;
mod validation;

pub use auth::{
    AuthResolver, AuthStatus, RequestAuth, SecretString, TokenExchange, TokenExchangeResult,
};
pub use cancellation::Cancellation;
pub use content::{
    AssistantContent, ImageContent, InputContent, TextContent, ThinkingContent, ToolCall,
};
pub use conversation::{Context, Message, ToolDeclaration, ToolResultMessage, UserMessage};
pub use events::ModelEvent;
pub use projection::project_context;
pub use provider::{
    Provider, ProviderDescription, ProviderOptions, ProviderStream, ProviderUpdate, StreamOptions,
};
pub use registry::Models;
pub use scripted::{Script, ScriptFactory, ScriptStep, ScriptedCall, ScriptedProvider};
pub use stream::ModelStream;
pub use types::{AssistantMessage, Failure, Model, ModelIdentity, StopReason, Usage};
pub use validation::{ToolValidationError, validate_tool_call};
