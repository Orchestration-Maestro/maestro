//! Canonical native write targets with beside-target exclusion.
use crate::{WriteLock, names::beside};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io,
    path::{Path, PathBuf},
};

/// Acquires canonical file-write exclusion and completes any interrupted publication.
///
/// Missing paths use their deepest existing canonical ancestor. The caller
/// selects whether to follow final symbolic links; authored paths are never modified.
///
/// # Errors
/// Returns native resolution, lock or recovery errors before admitting the writer.
pub fn acquire_write(path: &Path, through_symlinks: bool) -> io::Result<WriteLock> {
    acquire_with(
        path,
        through_symlinks,
        |file| file.lock().map_err(TryLockError::Error),
        Waiting::UntilFree,
    )
    .map_err(io::Error::from)
}

/// Makes at most ten attempts, waiting 20 ms only between contended attempts.
///
/// # Errors
/// Returns `TryLockError::WouldBlock` after ten contended attempts, or a native error unchanged.
pub fn acquire_write_bounded(
    path: &Path,
    through_symlinks: bool,
) -> Result<WriteLock, TryLockError> {
    crate::retry_contended(
        || try_acquire_write(path, through_symlinks),
        std::thread::sleep,
    )
}

/// Attempts canonical file-write exclusion once, replaying recovery before returning.
///
/// # Errors
/// Returns `TryLockError::WouldBlock` on contention, or a native resolution, lock or recovery error.
pub fn try_acquire_write(path: &Path, through_symlinks: bool) -> Result<WriteLock, TryLockError> {
    acquire_with(path, through_symlinks, File::try_lock, Waiting::Once)
}

/// Whether a refused native open should wait for another holder to close.
#[derive(Clone, Copy)]
enum Waiting {
    /// Wait without a contention budget.
    UntilFree,
    /// Return contention to the caller's retry owner.
    Once,
}

/// Opens, locks and verifies a live name, reopening retired files without spending an attempt.
fn acquire_with(
    path: &Path,
    follow: bool,
    mut lock: impl FnMut(&File) -> Result<(), TryLockError>,
    waiting: Waiting,
) -> Result<WriteLock, TryLockError> {
    let target = canonical_target(path, follow).map_err(TryLockError::Error)?;
    let sidecar = beside(&target, "", ".lock");
    loop {
        // Reopening a retired file repeats the link check, including a link introduced during open.
        reject_link(&sidecar).map_err(TryLockError::Error)?;
        let file = open_lock(&sidecar, waiting)?;
        #[cfg(test)]
        crate::replace::tests::acquisition("opened");
        lock(&file)?;
        #[cfg(test)]
        crate::replace::tests::acquisition("locked");
        if !still_named(&file, &sidecar).map_err(TryLockError::Error)? {
            continue;
        }
        let guard = WriteLock {
            recovery: beside(&target, "", ".recovery"),
            target,
            #[cfg(not(windows))]
            sidecar,
            _file: file,
        };
        crate::recovery::replay(&guard).map_err(TryLockError::Error)?;
        return Ok(guard);
    }
}

/// Refuses a lock pathname that redirects native ownership onto another file.
fn reject_link(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock path is a symbolic link",
        ));
    }
    // Native open owns failures, including Windows delete-pending metadata denials.
    Ok(())
}

/// Native read, write and delete sharing keeps existing waiters valid until they close.
#[cfg(windows)]
const SHARE_ALL: u32 = 1 | 2 | 4;
/// Native deletion disposition applied when the final sharing handle closes.
#[cfg(windows)]
const DELETE_ON_CLOSE: u32 = 0x0400_0000;
/// Windows access denial, also returned for a delete-pending lock name.
#[cfg(windows)]
const ERROR_ACCESS_DENIED: i32 = 5;
/// Windows refusal caused by an incompatible open sharing mode.
#[cfg(windows)]
const ERROR_SHARING_VIOLATION: i32 = 32;

/// Opens the lock name with platform-specific release semantics.
fn options() -> OpenOptions {
    let mut open = OpenOptions::new();
    open.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        open.share_mode(SHARE_ALL).custom_flags(DELETE_ON_CLOSE);
    }
    open
}

/// Opens once on Unix, where contention is reported by the native file lock.
#[cfg(not(windows))]
fn open_lock(path: &Path, _waiting: Waiting) -> Result<File, TryLockError> {
    options().open(path).map_err(TryLockError::Error)
}

/// Retries delete-pending names, bounding access denials even for a waiting writer.
#[cfg(windows)]
fn open_lock(path: &Path, waiting: Waiting) -> Result<File, TryLockError> {
    let mut denials = 0;
    let mut missing_denial = false;
    loop {
        let error = match options().open(path) {
            Ok(file) => return Ok(file),
            Err(error) => error,
        };
        denials = if error.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
            denials + 1
        } else {
            0
        };
        let failure = match error.raw_os_error() {
            Some(ERROR_ACCESS_DENIED) => denied_name(path, &mut missing_denial),
            Some(ERROR_SHARING_VIOLATION) => OpenFailure::Contended,
            _ => OpenFailure::Refuse,
        };
        match failure {
            OpenFailure::Reopen => continue,
            OpenFailure::Refuse => return Err(TryLockError::Error(error)),
            OpenFailure::Contended => (),
        }
        if matches!(waiting, Waiting::Once) {
            return Err(TryLockError::WouldBlock);
        }
        if denials >= 10 {
            return Err(TryLockError::Error(error));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Whether a refused Windows open belongs to a live holder or an inaccessible entry.
#[cfg(windows)]
enum OpenFailure {
    /// Spend a contended attempt or wait for a sharing handle to close.
    Contended,
    /// Retry an absent entry once without spending a contended attempt.
    Reopen,
    /// Preserve the native failure immediately.
    Refuse,
}

/// Distinguishes delete-pending names from directories and repeated missing-name denials.
#[cfg(windows)]
fn denied_name(path: &Path, missing: &mut bool) -> OpenFailure {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => OpenFailure::Refuse,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if std::mem::replace(missing, true) {
                OpenFailure::Refuse
            } else {
                OpenFailure::Reopen
            }
        }
        _ => OpenFailure::Contended,
    }
}

/// Rejects an unlinked inode after a previous holder removed its lock name.
#[cfg(unix)]
fn still_named(file: &File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let held = file.metadata()?;
    match fs::symlink_metadata(path) {
        Ok(named) => Ok(held.dev() == named.dev() && held.ino() == named.ino()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// A delete-pending file cannot be reopened even through an already-locked handle.
#[cfg(windows)]
fn still_named(_file: &File, path: &Path) -> io::Result<bool> {
    use std::os::windows::fs::OpenOptionsExt;
    reject_link(path)?;
    match OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(SHARE_ALL)
        .open(path)
    {
        Ok(_) => Ok(true),
        Err(error)
            if matches!(
                error.raw_os_error(),
                Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
            ) || error.kind() == io::ErrorKind::NotFound =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

/// Other platforms retain their native locking behaviour.
#[cfg(not(any(unix, windows)))]
fn still_named(_file: &File, _path: &Path) -> io::Result<bool> {
    Ok(true)
}
/// Canonicalizes the selected entry, retaining any missing suffix.
fn canonical_target(path: &Path, follow: bool) -> io::Result<PathBuf> {
    let target = if follow {
        resolve_write_target(path)?
    } else {
        path.to_path_buf()
    };
    match fs::symlink_metadata(&target) {
        Ok(meta) if !meta.file_type().is_symlink() => return fs::canonicalize(&target),
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    let mut parent = target.parent();
    while let Some(ancestor) = parent {
        let directory = if ancestor.as_os_str().is_empty() {
            Path::new(".")
        } else {
            ancestor
        };
        match fs::canonicalize(directory) {
            Ok(canonical) => {
                return Ok(canonical.join(target.strip_prefix(ancestor).map_err(io::Error::other)?));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => parent = ancestor.parent(),
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(
        "replacement target has no existing ancestor",
    ))
}

/// Resolves at most forty final-component links, including dangling targets.
pub(crate) fn resolve_write_target(path: &Path) -> io::Result<PathBuf> {
    maestro_path::resolve_final_links(path, |current| match fs::symlink_metadata(current) {
        Ok(meta) if meta.file_type().is_symlink() => fs::read_link(current).map(Some),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    })
}
