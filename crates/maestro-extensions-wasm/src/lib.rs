//! Author-side contract for native-async extension components.
//!
//! Rust records define the event payloads and results; the interface files in `wit/` define
//! how callbacks are invoked and the host resources lent to them. An event crosses the
//! component boundary as one JSON document beside its owned resources, and this crate turns it
//! into the author's typed event. The component adapter is written against the host's imports,
//! so its functions run unchanged in a component and against an in-process host.

#[macro_use]
mod port;
mod agent;
#[doc(hidden)]
pub mod bindings;
mod compaction;
#[cfg(any(test, target_arch = "wasm32"))]
mod component_adapter;
mod loader;
mod messages;
mod system_prompt;
#[cfg(test)]
mod tests;
mod types;

pub use agent::*;
pub use compaction::*;
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use component_adapter::Glue;
pub use loader::{Extension, ExtensionFactory, load_extension_from_factory};
pub use maestro_request::diagnostics::{
    AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo,
};
pub use maestro_request::skills::Skill;
pub use maestro_request::source_info::{SourceInfo, SourceOrigin, SourceScope};
pub use maestro_request::types::{
    AnthropicMessagesCompat, Api, AssistantContent, AssistantMessage, AssistantMessageEvent,
    CacheControlFormat, DataCollection, DoneReason, ErrorReason, ImageContent, MaxPrice,
    MaxTokensField, Message, Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel,
    OpenAICompletionsCompat, OpenAIResponsesCompat, OpenRouterRouting, Provider, RoutingPrice,
    RoutingSort, RoutingThreshold, SharedAssistantMessage, StopReason, TextContent,
    ThinkingContent, ThinkingFormat, ThinkingLevelMap, ToolCall, ToolResultMessage, Usage,
    UsageCost, UserBlock, UserContent, UserMessage, VercelGatewayRouting,
};
pub use messages::*;
pub use system_prompt::*;
pub use types::*;

#[cfg(test)]
extern crate self as maestro_extensions_wasm;
