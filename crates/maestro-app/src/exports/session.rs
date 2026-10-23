//! Existing session owners exposed at the application root.

pub use maestro_session::session_manager::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, LabelEntry,
    ModelChangeEntry, SessionEntry, SessionEntryBase, SessionInfoEntry, SessionMessageEntry,
    SessionTreeNode, ThinkingLevelChangeEntry,
};

pub use maestro_session::{Presence, SessionInfo};

pub use maestro_session::messages::{
    BranchSummaryMessage, CompactionSummaryMessage, parse_timestamp_millis,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::session_manager::SessionListProgress;

pub use maestro_session::export_html;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::agent_session::{
    AgentSession, AgentSessionConfig, AgentSessionEvent, AgentSessionEventListener,
    ModelCycleResult, ParsedSkillBlock, SessionStats, parse_skill_block,
};

// The session constructor's supplied owners, so a frontend can build a session.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_credentials::{ConfigValueOperations, ProcessConfigValueOperations};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_extensions::runner::ExtensionRunnerSession;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::session_manager::CompactionOptions;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
