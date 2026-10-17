//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod browser;
pub mod configuration;
pub mod presentation_data;
pub mod utils;

pub use maestro_models::{ModelCost, Usage, UsageCost};
pub use maestro_text::markup;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    BashOperations, BashRenderState, BashSpawnContext, BashSpawnHook, BashToolDetails,
    BashToolInput, BashToolOptions, create_bash_tool, create_bash_tool_definition,
    create_local_bash_operations, utils::shell::get_shell_config,
};

pub use maestro_tools::output::render_utils::{ToolRenderResultLike, get_text_output};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{EditRenderState, ToolDef};
pub use maestro_tools::{
    ToolDefinition, ToolRenderCall, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    ToolRenderResult, ToolRenderResultBinding, ToolRenderResultCallback, ToolRenderResultOptions,
    ToolRenderShell, ToolRendererBinding, ToolRendererState,
};

pub use presentation_data::footer_data_provider::ReadonlyFooterDataProvider;

pub use maestro_json::{canonical_entries, is_truthy};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_process::{
    Launch, OutputMode, SpawnSyncError, SpawnSyncOptions, spawn, spawn_sync, windows_shell_launch,
};
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
    Message, Model, ModelInput, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface, ProviderStreamOptions, SharedAssistantMessage, SimpleStreamOptions,
    StopReason, TextContent, UserBlock, UserContent, default_fetch,
};
pub use maestro_session::messages::{
    BranchSummaryMessage, CompactionSummaryMessage, parse_timestamp_millis,
};
pub use maestro_text::number_format::LocaleNumberFormat;

/// Agent reasoning levels, including disabled reasoning.
pub use maestro_agent::ThinkingLevel;
/// Shared with the terminal frontend.
pub use maestro_json::{BorrowedJson, compact_json, compact_members_with_default};
pub use maestro_text::decode_utf8;
/// Shared with the terminal frontend.
pub use maestro_text::truncate_utf16;
/// Shared with the terminal frontend.
pub use maestro_tools::output::render_utils::format_read_line_range;
pub use maestro_tools::{VisualTruncateResult, truncate_to_visual_lines};

pub use maestro_credentials::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, AuthStorage, AuthStorageBackend,
    InMemoryAuthStorageBackend, OAuthCredential,
};
pub use maestro_models::Transport;
pub use maestro_settings::{
    DoubleEscapeAction, MessageDeliveryMode, TransportSetting, TreeFilterMode, WarningSettings,
};
#[cfg(not(target_arch = "wasm32"))]
pub use utils::clipboard::{copy_to_clipboard, copy_to_clipboard_with};

pub use browser::tools::extract_document::{EXTRACT_DOCUMENT_TOOL, create_extract_document_tool};
pub use maestro_catalog::{
    CatalogError, ModelFileOperations, ModelRegistry, ProviderConfigInput, ProviderModelInput,
    ScopedModel, model_id_comparator,
};
pub use maestro_models::models_are_equal;
pub use maestro_settings::{PackageSource, SettingsListEntry, SettingsManager};

pub use browser::storage::{
    AppStorage, AutoDiscoveryProviderType, CustomProvider, CustomProviderType,
    CustomProvidersStore, IndexConfig, IndexedDBConfig, ManualProviderType, ProviderKeysStore,
    SessionData, SessionMetadata, SessionsStore, SettingsStore, StorageBackend, StorageTransaction,
    Store, StoreConfig, get_app_storage, set_app_storage,
};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_resources::{NativeResourceOperations, SourceOrigin, SourceScope};
pub use maestro_resources::{
    ResourceEntry, ResourceFileType, ResourceOperations, canonicalize_path,
};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::session_manager::SessionListProgress;
pub use maestro_text::normalize_to_lf;
pub use maestro_tools::output::render_utils::strip_ansi;
pub use maestro_tools::{TruncationOptions, TruncationResult, truncate_tail};

pub use maestro_tools::keybinding_hints::plain_key_hint;
pub use maestro_tools::output::render_utils::num;

/// Sandbox SDK provider contract.
pub use browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
/// Sandbox SDK download, result and URL callback records.
pub use browser::components::sandboxed_iframe::{SandboxFile, SandboxResult, SandboxUrlProvider};
/// JavaScript execution tool factory.
pub use browser::tools::javascript_repl::create_javascript_repl_tool;
pub use maestro_json::pretty_json;
pub use maestro_tools::utils::image_convert::{ConvertedImage, convert_to_png};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    EditToolOptions, ReadToolOptions, WriteToolOptions, create_edit_tool_definition,
    create_read_tool_definition, create_write_tool_definition,
};

pub use browser::tools::artifacts::{Artifact, ArtifactsPanel, ArtifactsParams};
pub use browser::tools::prompts::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO, ATTACHMENTS_RUNTIME_DESCRIPTION,
};
pub use maestro_session::export_html;

pub use configuration::config::VERSION;
#[cfg(not(target_arch = "wasm32"))]
pub use configuration::config::get_agent_dir;

/// Configurable application action identifiers and their shared manager.
pub use presentation_data::keybindings::{AppKeybinding, KeybindingsManager};

#[cfg(not(target_arch = "wasm32"))]
pub mod output_guard;

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_packages::{
    DefaultPackageManager, PackageManager, PathMetadata, ProgressCallback, ProgressEvent,
    ResolvedPaths, ResolvedResource,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_models::default_web_socket;
pub use maestro_models::{
    ConnectWebSocket, WebSocket, WebSocketConnection, WebSocketFuture, WebSocketRequest,
};
pub use presentation_data::slash_commands::{SlashCommandInfo, SlashCommandSource};
