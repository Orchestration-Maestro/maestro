//! Supplied-path snapshot storage and load diagnostics.
use super::snapshot::{SessionHistorySnapshot, SessionSnapshot};
use maestro_lock::{Replace, replace};
use std::{fs, io, path::Path};

/// Saves topology before saving or removing separate history.
///
/// # Errors
/// Returns serialization or precommit filesystem errors; a history error does
/// not undo an already committed topology.
pub fn save_to_paths(
    session_path: &Path,
    history_path: &Path,
    snapshot: &SessionSnapshot,
    history: Option<&SessionHistorySnapshot>,
) -> io::Result<()> {
    save_json_to_path(session_path, snapshot)?;
    match history {
        Some(history) => save_json_to_path(history_path, history),
        None => clear_path(history_path),
    }
}
/// Serializes once and delegates byte publication to the lock leaf.
fn save_json_to_path<T: serde::Serialize>(path: &Path, snapshot: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(snapshot)?;
    replace(
        path,
        &bytes,
        Replace {
            through_symlinks: true,
            ..Replace::default()
        },
    )?;
    Ok(())
}
/// Removes only the supplied pathname, tolerating absence.
///
/// # Errors
/// Returns native lock, recovery and removal errors other than an absent parent or file.
pub fn clear_path(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if path.as_os_str().is_empty() || !parent.try_exists()? {
        return Ok(());
    }
    let holder = maestro_lock::acquire_write(path, false)?;
    match fs::remove_file(holder.path()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
use super::snapshot::{
    SNAPSHOT_VERSION, parse_history_snapshot, parse_snapshot, snapshot_file_version,
};
use tracing::warn;
/// Loads current topology, warning about read, parse or version failures.
#[must_use]
pub fn load(path: &Path) -> Option<SessionSnapshot> {
    if !path.exists() {
        return None;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            warn!(err = %err, "failed to read session file");
            return None;
        }
    };
    match parse_snapshot(&content) {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            if let Some(version) = snapshot_file_version(&content)
                && version > SNAPSHOT_VERSION
            {
                warn!(
                    file_version = version,
                    supported = SNAPSHOT_VERSION,
                    "session file is from a newer maestro version, ignoring"
                );
                return None;
            }
            warn!(err = %err, "failed to parse session file, ignoring");
            None
        }
    }
}

/// Loads separate current history with scoped failure diagnostics.
#[must_use]
pub fn load_history(path: &Path) -> Option<SessionHistorySnapshot> {
    if !path.exists() {
        return None;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            warn!(err = %err, "failed to read session history file");
            return None;
        }
    };
    match parse_history_snapshot(&content) {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            if let Some(version) = snapshot_file_version(&content)
                && version > SNAPSHOT_VERSION
            {
                warn!(
                    file_version = version,
                    supported = SNAPSHOT_VERSION,
                    "session history file is from a newer maestro version, ignoring"
                );
                return None;
            }
            warn!(err = %err, "failed to parse session history file, ignoring");
            None
        }
    }
}
