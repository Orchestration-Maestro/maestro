//! Runtime-independent model access through explicitly registered adapters.
//! Indexed text, thinking and tool-call updates become independent owned
//! snapshots with strict block validation and exactly one terminal outcome.
//! Completion drains the same stream. Supplied cancellation wakes blocked reads
//! and releases local work without claiming to undo remote effects.
//! Selected-provider request authentication is supplied explicitly or resolved
//! once; credential lifecycle remains outside this crate.

#![doc = include_str!("../../../docs/models.md")]
#![doc = include_str!("../../../docs/request-authentication.md")]

mod auth;
mod cancellation;
mod content;
mod dispatch;
mod events;
mod provider;
mod registry;
mod scripted;
mod stream;
mod types;

pub use auth::{
    AuthResolver, AuthStatus, RequestAuth, SecretString, TokenExchange, TokenExchangeResult,
};
pub use cancellation::Cancellation;
pub use content::{AssistantContent, TextContent, ThinkingContent, ToolCall};
pub use events::ModelEvent;
pub use provider::{
    Provider, ProviderDescription, ProviderOptions, ProviderStream, ProviderUpdate, StreamOptions,
};
pub use registry::Models;
pub use scripted::{Script, ScriptFactory, ScriptStep, ScriptedCall, ScriptedProvider};
pub use stream::ModelStream;
pub use types::{
    AssistantMessage, Context, Failure, Model, ModelIdentity, StopReason, Usage, UserMessage,
};
