//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod browser;
pub mod presentation_data;

pub use maestro_models::{ModelCost, Usage, UsageCost};
pub use maestro_text::markup;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    BashOperations, create_local_bash_operations, utils::shell::get_shell_config,
};

pub use presentation_data::footer_data_provider::ReadonlyFooterDataProvider;

pub use maestro_json::is_truthy;
pub use maestro_session::session_manager::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, LabelEntry,
    ModelChangeEntry, SessionEntry, SessionEntryBase, SessionInfoEntry, SessionMessageEntry,
    SessionTreeNode, ThinkingLevelChangeEntry,
};
pub use maestro_session::{Presence, SessionInfo};
pub use maestro_text::is_whitespace_scalar;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_process::{SpawnSyncError, SpawnSyncOptions, StderrMode, spawn, spawn_sync};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::process::exec::{ExecOptions, ExecResult, exec_command};
pub use maestro_tools::{RenderDiffOptions, key_hint, key_text, raw_key_hint, render_diff};
