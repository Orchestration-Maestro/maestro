//! Backup and publication of application keybinding resets.
use serde_json::{Map, Value};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// A completed reset and any postcommit durability warning.
#[derive(Debug)]
pub struct KeybindingsReset {
    /// Exclusive timestamped backup, or `None` when no overrides needed removal.
    pub backup_path: Option<PathBuf>,
    /// Number of multiplexer overrides removed.
    pub removed: usize,
    /// Parent-directory sync failure after successful publication.
    pub dir_sync: Option<io::Error>,
}

/// Back up the supplied root's keybindings before removing multiplexer overrides.
///
/// `timestamp` is the caller's observed Unix time in seconds. Existing backups are
/// never overwritten. Chat actions and unknown members retain their JSON values.
/// Missing files and documents without multiplexer overrides succeed without writes.
///
/// # Errors
/// Returns a read, JSON object validation, backup or precommit publication error.
/// An error before publication leaves the original unchanged; a publication
/// interrupted after it starts is completed by the next writer from its recovery copy.
pub fn reset_multiplexer_keybindings(
    agent_dir: &str,
    timestamp: u64,
) -> io::Result<KeybindingsReset> {
    let path = Path::new(agent_dir).join("keybindings.json");
    let unchanged = KeybindingsReset {
        backup_path: None,
        removed: 0,
        dir_sync: None,
    };
    let original = match fs::read_to_string(&path) {
        Ok(original) => original,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(unchanged),
        Err(error) => return Err(error),
    };
    let mut raw: Map<String, Value> = maestro_json::object::from_str(&original)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let before = raw.len();
    raw.retain(|key, _| !key.starts_with("multiplexer."));
    if before == raw.len() {
        return Ok(unchanged);
    }
    let mut updated = serde_json::to_string_pretty(&raw).map_err(io::Error::other)?;
    updated.push('\n');
    let backup_path = path.with_file_name(format!("keybindings.json.bak-{timestamp}"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut backup = options.open(&backup_path)?;
    backup.write_all(original.as_bytes())?;
    backup.sync_all()?;
    drop(backup);
    let committed = maestro_lock::replace(
        &path,
        updated.as_bytes(),
        maestro_lock::Replace {
            through_symlinks: true,
            creation_mode: None,
        },
    )?;
    Ok(KeybindingsReset {
        backup_path: Some(backup_path),
        removed: before - raw.len(),
        dir_sync: committed.dir_sync,
    })
}
