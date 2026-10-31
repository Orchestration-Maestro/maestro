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
/// Existing permission bits are retained. Hard links, ownership and ACLs are not preserved.
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
    #[cfg(test)]
    crate::replace::tests::before_create(&temporary)?;
    let file = File::create(&temporary)?;
    let result = retain_permissions(&file, &target)
        .and_then(|()| write_temporary(file, bytes))
        .and_then(|()| {
            #[cfg(test)]
            crate::replace::tests::effect("rename")?;
            fs::rename(&temporary, &target)
        });
    if let Err(error) = result {
        // Keep the publication error if cleanup also fails.
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(Committed {
        dir_sync: sync_parent_dir(parent).err(),
    })
}

/// Keeps existing access bits before any intended bytes reach the temporary file.
fn retain_permissions(file: &File, target: &Path) -> io::Result<()> {
    match fs::metadata(target) {
        Ok(metadata) => file.set_permissions(metadata.permissions()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Writes and syncs all bytes before closing the temporary handle.
fn write_temporary(mut file: File, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    crate::replace::tests::effect("write_all")?;
    file.write_all(bytes)?;
    #[cfg(test)]
    crate::replace::tests::effect("file_sync")?;
    file.sync_all()
}

/// Selects the caller's fixed extension or a hidden per-publication name.
pub(crate) fn temporary_path(target: &Path, temporary: Temporary<'_>) -> PathBuf {
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
