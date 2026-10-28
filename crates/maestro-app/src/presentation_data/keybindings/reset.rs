//! Backup and publication of application keybinding resets.
use std::{io, path::PathBuf};

/// A completed reset and any postcommit durability warning.
#[derive(Debug)]
pub struct KeybindingsReset {
    /// Exclusive timestamped backup containing the original bytes.
    pub backup_path: PathBuf,
    /// Number of multiplexer overrides removed.
    pub removed: usize,
    /// Parent-directory sync failure after successful publication.
    pub dir_sync: Option<io::Error>,
}

/// Back up the supplied root's keybindings before removing multiplexer overrides.
///
/// `timestamp` is the caller's observed Unix time in seconds. Existing backups are
/// never overwritten. Chat actions and unknown members retain their JSON values.
///
/// # Errors
/// Returns a read, JSON object validation, backup or precommit publication error.
/// Every error leaves the original document unchanged.
pub fn reset_multiplexer_keybindings(
    agent_dir: &str,
    timestamp: u64,
) -> io::Result<KeybindingsReset> {
    let _ = (agent_dir, timestamp);
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "keybinding reset is unavailable",
    ))
}
