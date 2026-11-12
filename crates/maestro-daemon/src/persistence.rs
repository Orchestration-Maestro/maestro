//! Snapshot effects through the shared workspace storage owner.

use crate::{diagnostics, locations::DaemonPaths};
use maestro_workspace::persistence::records::{SessionHistorySnapshot, SessionSnapshot};
use maestro_workspace::persistence::{clear_path, save_to_paths};
use std::path::Path;

#[cfg(target_os = "linux")]
mod capture;
#[cfg(target_os = "linux")]
pub use capture::{capture, capture_history};

/// Save topology before history, reporting only a completed combined save as success.
pub fn save(
    paths: &DaemonPaths,
    snapshot: &SessionSnapshot,
    history: Option<&SessionHistorySnapshot>,
) {
    let path = paths.snapshot_path();
    if let Err(err) = save_to_paths(
        Path::new(&path),
        Path::new(&paths.history_path()),
        snapshot,
        history,
    ) {
        diagnostics::session_save_failed(Path::new(&path), &err.to_string());
        return;
    }
    diagnostics::session_saved(Path::new(&path), snapshot.workspaces.len());
}

/// Clear topology first; leave history untouched when topology removal fails.
pub fn clear(paths: &DaemonPaths) {
    let path = paths.snapshot_path();
    if let Err(err) = clear_path(Path::new(&path)) {
        diagnostics::session_clear_failed(Path::new(&path), &err.to_string());
        return;
    }
    clear_history(paths);
    diagnostics::session_cleared(Path::new(&path));
}

/// Clear separate history, tolerating absence and reporting other removal failures.
pub fn clear_history(paths: &DaemonPaths) {
    let path = paths.history_path();
    if let Err(err) = clear_path(Path::new(&path)) {
        diagnostics::session_clear_failed(Path::new(&path), &err.to_string());
    }
}

#[cfg(test)]
mod tests;
