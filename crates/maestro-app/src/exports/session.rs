//! Existing session owners exposed at the application root.

pub use maestro_session::session_manager::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, LabelEntry,
    ModelChangeEntry, SessionEntry, SessionEntryBase, SessionInfoEntry, SessionMessageEntry,
    SessionTreeNode, ThinkingLevelChangeEntry,
};

pub use maestro_session::{Presence, SessionInfo, SessionManager};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::session_cwd::get_missing_session_cwd_issue;
pub use maestro_session::session_cwd::{
    MissingSessionCwdError, SessionCwdIssue, format_missing_session_cwd_prompt,
};

/// The session's message records, including the application roles a transcript shows.
pub use maestro_session::messages;

pub use maestro_session::messages::{
    BranchSummaryMessage, CompactionSummaryMessage, parse_timestamp_millis,
};

pub use maestro_session::messages::CustomMessage;

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::session_manager::SessionListProgress;

pub use maestro_session::export_html;

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_export::{
    ExportOperations, ExportOptions, NativeExportOperations, export_from_file,
};

pub use crate::skill_block::{ParsedSkillBlock, parse_skill_block};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::agent_session::{
    AgentSession, AgentSessionConfig, AgentSessionEvent, AgentSessionEventListener,
    ExtensionBindings, ModelCycleResult, NavigateTreeResult, PromptOptions, SessionStats,
    delivery_mode,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::runtime::agent_session_runtime::{
    AgentSessionRuntime, AgentSessionRuntimeError, BeforeSessionInvalidate,
    CreateAgentSessionRuntimeFactory, CreateAgentSessionRuntimeOptions,
    CreateAgentSessionRuntimeResult, ForkResult, RebindSession, SessionImportFileNotFoundError,
    create_agent_session_runtime,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::runtime::agent_session_services::{
    CreateAgentSessionFromServicesOptions, create_agent_session_from_services,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::sdk::{
    CreateAgentSessionOptions, CreateAgentSessionResult, NoTools, SessionResources,
    create_agent_session, queue_mode,
};

pub use maestro_session::compaction::{
    BranchPreparation, BranchSummaryResult, CollectEntriesResult, CompactionResult, CutPointResult,
    DEFAULT_COMPACTION_SETTINGS, FileOperations, GenerateBranchSummaryOptions, GeneratedSummary,
    calculate_context_tokens, collect_entries_for_branch_summary, compact, estimate_tokens,
    find_cut_point, find_turn_start_index, generate_branch_summary, generate_summary,
    generate_summary_with_usage, get_last_assistant_usage, prepare_branch_entries,
    serialize_conversation, should_compact,
};

pub use maestro_session::convert_to_llm;

pub use maestro_session::session_manager::NavigateTreeOptions;

pub use maestro_session::session_manager::{
    CURRENT_SESSION_VERSION, FileEntry, NewSessionOptions, SessionContext, SessionHeader,
    build_session_context, get_latest_compaction_entry, migrate_session_entries,
    parse_session_entries,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_extensions::runner::ExtensionRunnerSession;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::session_manager::CompactionOptions;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::session_manager::ReadonlySessionManager;
