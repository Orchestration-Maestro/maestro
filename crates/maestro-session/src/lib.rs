//! Session history and conversation projection into model requests.
pub mod messages;
pub use messages::convert_to_llm;

/// Session history and selected-branch context.
pub mod session_manager;
pub use session_manager::{
    BranchSummaryEntry, CURRENT_SESSION_VERSION, CompactionEntry, CustomEntry, CustomMessageEntry,
    FileEntry, ModelChangeEntry, NewSessionOptions, SessionContext, SessionEntry, SessionEntryBase,
    SessionHeader, SessionInfo, SessionInfoEntry, SessionManager, SessionMessageEntry,
    ThinkingLevelChangeEntry, build_session_context, get_latest_compaction_entry,
    migrate_session_entries, parse_session_entries,
};
#[cfg(not(target_arch = "wasm32"))]
pub use session_manager::{SessionDir, sessions_dir};

/// Missing stored working-directory diagnostics.
pub mod session_cwd;

pub use maestro_request::presence::Presence;

/// Compaction preparation, summary generation and shared input utilities.
pub mod compaction;
pub use compaction::{
    CompactionDetails, CompactionPreparation, CompactionSettings, ContextUsageEstimate,
    CutPointResult, DEFAULT_COMPACTION_SETTINGS, FileOperations, calculate_context_tokens,
    compute_file_lists, create_file_ops, estimate_context_tokens, estimate_tokens,
    extract_file_ops_from_message, find_cut_point, find_turn_start_index, format_file_operations,
    get_last_assistant_usage, prepare_compaction, serialize_conversation, should_compact,
};
pub use compaction::{
    CompactionResult, GeneratedSummary, SummaryOptions, compact, generate_summary,
    generate_summary_with_usage,
};

pub use compaction::{CollectEntriesResult, collect_entries_for_branch_summary};

pub use compaction::{BranchPreparation, prepare_branch_entries};

pub use compaction::{BranchSummaryResult, GenerateBranchSummaryOptions, generate_branch_summary};

/// Shared session export data preparation.
pub mod export_html;
