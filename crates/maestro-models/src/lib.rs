//! Explicit, credential-free model text access through replaceable adapters.
//! A caller supplies a clock, registers a model and consumes independent owned
//! snapshots. Text streams emit start, text-start, deltas, text-end and done;
//! failures terminate with a typed secret-safe error. No runtime is required by
//! this library; callers supply an executor for asynchronous consumption.

#![doc = include_str!("../../../docs/models.md")]

mod provider;
mod registry;
mod scripted;
mod stream;
mod types;

pub use provider::{Provider, ProviderStream, ProviderUpdate};
pub use registry::Models;
pub use scripted::ScriptedProvider;
pub use stream::ModelStream;
pub use types::{
    AssistantMessage, Context, Failure, Model, ModelEvent, ModelIdentity, StopReason, TextContent,
    Usage, UserMessage,
};
