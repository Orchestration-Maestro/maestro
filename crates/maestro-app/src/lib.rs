//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod browser;
pub mod presentation_data;

pub use maestro_models::{ModelCost, Usage, UsageCost};
pub use maestro_text::markup;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    BashOperations, BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput,
    BashToolOptions, create_bash_tool, create_bash_tool_definition, create_local_bash_operations,
    utils::shell::get_shell_config,
};

pub use presentation_data::footer_data_provider::ReadonlyFooterDataProvider;

pub use maestro_json::is_truthy;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_process::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn, spawn_sync};
pub use maestro_session::session_manager::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, LabelEntry,
    ModelChangeEntry, SessionEntry, SessionEntryBase, SessionInfoEntry, SessionMessageEntry,
    SessionTreeNode, ThinkingLevelChangeEntry,
};
pub use maestro_session::{Presence, SessionInfo};
pub use maestro_text::is_whitespace_scalar;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::process::exec::{ExecOptions, ExecResult, exec_command};
pub use maestro_tools::{RenderDiffOptions, key_hint, key_text, raw_key_hint, render_diff};

pub use maestro_agent::{AgentMessage, CustomAgentMessages, StreamFn};
pub use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEventStream, BoxFuture, Context,
    DiagnosticErrorInfo, Fetch, FetchError, HttpBody, HttpRequest, HttpResponse, ImageContent,
    Message, Model, ModelInput, ProviderStreamOptions, SharedAssistantMessage, SimpleStreamOptions,
    StopReason, TextContent, UserBlock, UserContent, default_fetch,
};
pub use maestro_session::messages::{BranchSummaryMessage, CompactionSummaryMessage};
pub use maestro_text::number_format::LocaleNumberFormat;

/// Agent reasoning levels, including disabled reasoning.
pub use maestro_agent::ThinkingLevel;
