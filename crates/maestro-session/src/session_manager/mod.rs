//! Supplied session records and selected-branch context.
/// Context projection operations.
pub mod context;
/// Session record views.
pub mod types;
pub use context::{build_session_context, get_latest_compaction_entry, parse_session_entries};
pub use types::*;
