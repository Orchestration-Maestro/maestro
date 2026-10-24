//! Current-format workspace persistence at caller-supplied paths.

/// Native path storage and load diagnostics.
pub mod io;
/// Owned topology and screen-history records.
pub mod records;

pub use io::{clear_path, load, load_history, save_to_paths};
pub use records::{
    DirectionSnapshot, LayoutSnapshot, SessionHistorySnapshot, SessionSnapshot, TabSnapshot,
    WorkspaceSnapshot,
};
