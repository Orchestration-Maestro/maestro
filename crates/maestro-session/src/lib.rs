//! Stored-entry context and conversation projection into model requests.
pub mod messages;
pub use messages::convert_to_llm;

/// Supplied session records and selected-branch context.
pub mod session_manager;
pub use session_manager::{
    BranchSummaryEntry, CURRENT_SESSION_VERSION, CompactionEntry, CustomEntry, CustomMessageEntry,
    FileEntry, ModelChangeEntry, SessionContext, SessionEntry, SessionEntryBase, SessionHeader,
    SessionInfoEntry, SessionMessageEntry, ThinkingLevelChangeEntry, build_session_context,
    get_latest_compaction_entry, parse_session_entries,
};
