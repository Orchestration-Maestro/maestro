//! Existing session owners exposed at the application root.

pub use maestro_session::session_manager::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, ModelChangeEntry,
    SessionEntry, SessionEntryBase, SessionInfoEntry, SessionMessageEntry, SessionTreeNode,
    ThinkingLevelChangeEntry,
};

pub use maestro_session::{SessionInfo, SessionManager};

pub use crate::skill_block::{ParsedSkillBlock, parse_skill_block};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::agent_session::{
    AgentSession, AgentSessionConfig, AgentSessionEvent, AgentSessionEventListener,
    ExtensionBindings, ModelCycleResult, NavigateTreeResult, PromptOptions, SessionStats,
    delivery_mode,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::runtime::session_engine::{
    AgentSessionRuntime, AgentSessionRuntimeError, BeforeSessionInvalidate,
    CreateAgentSessionRuntimeFactory, CreateAgentSessionRuntimeOptions,
    CreateAgentSessionRuntimeResult, ForkResult, RebindSession, SessionImportFileNotFoundError,
    create_agent_session_runtime,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::runtime::session_dependencies::{
    CreateAgentSessionFromServicesOptions, create_agent_session_from_services,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::sdk::{
    CreateAgentSessionOptions, CreateAgentSessionResult, NoTools, SessionResources,
    create_agent_session, queue_mode,
};

pub use maestro_session::summarization::{
    BranchPreparation, BranchSummaryResult, CollectEntriesResult, CompactionResult, CutPointResult,
    DEFAULT_COMPACTION_SETTINGS, FileOperations, GenerateBranchSummaryOptions,
    calculate_context_tokens, collect_entries_for_branch_summary, compact, estimate_tokens,
    find_cut_point, find_turn_start_index, generate_branch_summary, generate_summary,
    generate_summary_with_usage, get_last_assistant_usage, prepare_branch_entries,
    serialize_conversation, should_compact,
};

pub use maestro_session::convert_to_llm;

pub use maestro_session::session_manager::{
    CURRENT_SESSION_VERSION, FileEntry, NewSessionOptions, SessionContext, SessionHeader,
    build_session_context, get_latest_compaction_entry, migrate_session_entries,
    parse_session_entries,
};
