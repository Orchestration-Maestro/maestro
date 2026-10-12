//! In-memory history and conversation projection into model requests.
pub mod messages;
pub use messages::convert_to_llm;

/// In-memory session history and selected-branch context.
pub mod session_manager;
pub use session_manager::{
    BranchSummaryEntry, CURRENT_SESSION_VERSION, CompactionEntry, CustomEntry, CustomMessageEntry,
    FileEntry, ModelChangeEntry, NewSessionOptions, SessionContext, SessionEntry, SessionEntryBase,
    SessionHeader, SessionInfoEntry, SessionManager, SessionMessageEntry, ThinkingLevelChangeEntry,
    build_session_context, get_latest_compaction_entry, parse_session_entries,
};
