//! Atomic byte publication for readers that do not hold a write lock.
use crate::{Committed, identity::resolve_write_target, replace::sync_parent_dir};
use std::{
    fs::{self, File, OpenOptions},
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
/// Writers share canonical target exclusion. Stale fixed temporaries are unlinked
/// before exclusive creation. Existing permission bits are retained from creation
/// on Unix, except that a read-only Unix target is replaced by a file with the
/// default creation mode. A read-only Windows target is refused by the rename.
/// Hard links, ownership and ACLs are not preserved.
///
/// # Errors
/// Returns native resolution, lock, recovery, creation, write, sync or rename errors.
/// A fixed temporary extension that names the target returns `InvalidInput`.
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
    let _guard = crate::acquire_write(&target, false)?;
    let temporary = prepare_temporary(&target, temporary)?;
    #[cfg(test)]
    crate::replace::tests::before_create(&temporary)?;
    let kept = retained_permissions(&target)?;
    let file = create_temporary(&temporary, kept.as_ref())?;
    #[cfg(test)]
    crate::replace::tests::created();
    let result = retain_permissions(&file, kept)
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

/// Removes stale fixed entries under exclusion without following their links.
fn prepare_temporary(target: &Path, policy: Temporary<'_>) -> io::Result<PathBuf> {
    let temporary = temporary_path(target, policy);
    if temporary == target {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "temporary path names the publication target",
        ));
    }
    if matches!(policy, Temporary::Fixed(_)) {
        match fs::remove_file(&temporary) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            result => result?,
        }
    }
    Ok(temporary)
}

/// The target's permissions that the published file keeps; a read-only Unix target keeps none.
fn retained_permissions(target: &Path) -> io::Result<Option<fs::Permissions>> {
    match fs::metadata(target) {
        Ok(metadata) if cfg!(unix) && metadata.permissions().readonly() => Ok(None),
        Ok(metadata) => Ok(Some(metadata.permissions())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Exclusively creates a temporary with no initial Unix access beyond the kept permissions.
fn create_temporary(temporary: &Path, kept: Option<&fs::Permissions>) -> io::Result<File> {
    let mut open = OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        open.mode(kept.map_or(0o666, PermissionsExt::mode));
    }
    #[cfg(not(unix))]
    let _ = kept;
    open.open(temporary)
}

/// Keeps the retained access bits before any intended bytes reach the temporary file.
fn retain_permissions(file: &File, kept: Option<fs::Permissions>) -> io::Result<()> {
    kept.map_or(Ok(()), |permissions| file.set_permissions(permissions))
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
            crate::names::beside(target, ".", &format!(".{}.{nanos}.tmp", std::process::id()))
        }
    }
}
