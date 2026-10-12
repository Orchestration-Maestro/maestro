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

/// Missing stored working-directory diagnostics.
pub mod session_cwd;
