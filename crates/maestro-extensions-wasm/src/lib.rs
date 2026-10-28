//! Author-side contract for native-async extension components.
//!
//! Rust records define the event payloads and results; the interface files in `wit/` define
//! how callbacks are invoked and the host resources lent to them. An event crosses the
//! component boundary as one JSON document beside its owned resources, and this crate turns it
//! into the author's typed event. The component adapter is written against the host's imports,
//! so its functions run unchanged in a component and against an in-process host.

#[macro_use]
mod port;
mod abort_controller;
mod agent;
pub use abort_controller::AbortController;
pub mod utils;
#[cfg(target_arch = "wasm32")]
pub use utils::event_stream::create_assistant_message_event_stream;
pub use utils::event_stream::{AssistantMessageEventStream, EventStream, EventStreamPort};
mod autocomplete;
pub use autocomplete::*;
mod editor_component;
pub use editor_component::{
    CustomEditorComponent, EditorComponent, EditorFactory, EditorFactoryOutcome,
};
mod event_bus;
pub use event_bus::*;
mod bash_executor;
pub mod tools;
pub use bash_executor::*;
pub use tools::{
    ALL_TOOL_NAMES, BashExec, BashExecOptions, BashExecResult, BashOnData, BashOperations,
    BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput, BashToolOptions,
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, DetectImageMimeType, EditOperations, EditToolDetails,
    EditToolInput, EditToolOptions, FindGlobOptions, FindOperations, FindToolDetails,
    FindToolInput, FindToolOptions, GrepOperations, GrepToolDetails, GrepToolInput,
    GrepToolOptions, LsOperations, LsToolDetails, LsToolInput, LsToolOptions, OperationFuture,
    ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, ReplaceEdit, ToolDef,
    ToolName, ToolsOptions, TruncationLimit, TruncationOptions, TruncationResult, WriteOperations,
    WriteToolInput, WriteToolOptions, format_size, truncate_head, truncate_line, truncate_tail,
};
#[doc(hidden)]
pub mod bindings;
/// Native child resources.
pub mod child_process;
pub mod compaction;
mod exec;
/// Native filesystem notifications.
pub mod fs;
/// Native HTTP resources.
pub mod http;
/// Native operation capability.
pub mod io;
/// Raw byte resources.
pub mod streams;
pub use exec::{ExecOptions, ExecResult};
#[cfg(any(test, target_arch = "wasm32"))]
mod component_adapter;
mod loader;
mod messages;
mod slash_commands;
pub use slash_commands::*;
mod oauth;
pub use oauth::*;
mod auth_storage;
pub use auth_storage::*;
mod model_registry;
mod session_manager;
pub use model_registry::*;
mod system_prompt;
#[cfg(test)]
mod tests;

/// Native terminal key vocabulary and decoding.
pub mod keys;
pub use keys::{
    Key, KeyEventType, KeyId, decode_kitty_printable, is_key_release, is_key_repeat,
    is_kitty_protocol_active, matches_key, parse_key, set_kitty_protocol_active,
};
/// Native fuzzy matching over original caller-owned items.
pub mod fuzzy;
pub use fuzzy::{FuzzyMatch, fuzzy_filter, fuzzy_match};
/// Native metadata parsing with complete typed values.
pub mod frontmatter;
pub use frontmatter::{parse_frontmatter, parse_yaml, strip_frontmatter};
/// Native application installation identity.
pub mod config;
mod tui;
mod types;
pub use tui::*;
pub use utils::{truncate_to_width, visible_width, wrap_text_with_ansi};
mod theme;
pub use theme::*;

pub use agent::*;
pub use compaction::utils::serialize_conversation;
pub use compaction::{CompactionPreparation, CompactionResult, CompactionSettings, FileOperations};
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use component_adapter::Glue;
pub use loader::{Extension, ExtensionFactory, load_extension_from_factory};
pub use maestro_request::auth_storage::{AuthSource, AuthStatus};
pub use maestro_request::diagnostics::{
    AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo,
};
pub use maestro_request::parameter_schema::{ParameterSchema, SchemaKind};
pub use maestro_request::skills::Skill;
pub use maestro_request::source_info::{SourceInfo, SourceOrigin, SourceScope};
pub use maestro_request::types::{
    AnthropicMessagesCompat, Api, AssistantContent, AssistantMessage, AssistantMessageEvent,
    CacheControlFormat, Context, DataCollection, DoneReason, ErrorReason, ImageContent, MaxPrice,
    MaxTokensField, Message, Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel,
    OpenAICompletionsCompat, OpenAIResponsesCompat, OpenRouterRouting, Provider, RoutingPrice,
    RoutingSort, RoutingThreshold, SharedAssistantMessage, StopReason, TextContent,
    ThinkingContent, ThinkingFormat, ThinkingLevelMap, ToolCall, ToolResultMessage, Usage,
    UsageCost, UserBlock, UserContent, UserMessage, VercelGatewayRouting,
};
pub use messages::*;
pub use session_manager::*;
pub use system_prompt::*;
pub use types::*;

#[cfg(test)]
extern crate self as maestro_extensions_wasm;

/// Guest schema code using the shared record owner.
#[path = "types/schema_code.rs"]
pub mod schema_code;

#[cfg(any(test, target_arch = "wasm32"))]
mod schema_error;

/// Immediate model invocation and eager completion.
#[cfg(target_arch = "wasm32")]
pub mod stream;
#[cfg(target_arch = "wasm32")]
pub use stream::{complete, complete_simple, stream, stream_simple, stream_simple_via};

pub mod wasi;
