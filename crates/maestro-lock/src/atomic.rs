//! Atomic byte publication for readers that do not hold a write lock.
use crate::{Committed, identity::resolve_write_target, replace::sync_parent_dir};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Whether publication follows or replaces a final symbolic link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symlinks {
    /// Follow final links, including dangling targets.
    Follow,
    /// Replace the directory entry itself.
    Replace,
}

/// Temporary pathname used before the atomic rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Temporary<'a> {
    /// Replace the target extension with the supplied extension.
    Fixed(&'a str),
    /// Use a hidden name containing the target name, process ID and nanoseconds.
    Unique,
}

/// Writes and syncs temporary bytes, renames over the target, then syncs its parent.
///
/// Missing parents are created. Precommit failures remove the temporary file;
/// parent-sync failures after rename are returned as a committed warning.
/// This replaces the target inode rather than preserving hard links or access controls.
///
/// # Errors
/// Returns native link-resolution, directory-creation, temporary-write, sync or rename errors.
pub fn atomic_replace(
    path: &Path,
    bytes: &[u8],
    symlinks: Symlinks,
    temporary: Temporary<'_>,
) -> io::Result<Committed> {
    let target = match symlinks {
        Symlinks::Follow => resolve_write_target(path)?,
        Symlinks::Replace => path.to_path_buf(),
    };
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(&target, temporary);
    let mut file = File::create(&temporary)?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result.and_then(|()| fs::rename(&temporary, &target)) {
        // Keep the publication error if cleanup also fails.
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(Committed {
        dir_sync: sync_parent_dir(parent).err(),
    })
}

/// Selects the caller's fixed extension or a hidden per-publication name.
fn temporary_path(target: &Path, temporary: Temporary<'_>) -> PathBuf {
    match temporary {
        Temporary::Fixed(extension) => target.with_extension(extension),
        Temporary::Unique => {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let name = target.file_name().unwrap_or_default().to_string_lossy();
            target.with_file_name(format!(".{name}.{}.{nanos}.tmp", std::process::id()))
        }
    }
}
