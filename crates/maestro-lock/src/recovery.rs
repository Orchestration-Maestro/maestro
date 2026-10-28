//! Durable roll-forward bytes for an exclusively held canonical target.
use crate::WriteLock;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// Identity, optional-mode flag and mode preceding the intended target bytes.
const HEADER_LENGTH: usize = 37;

/// Publishes intended bytes before touching the original file's identity or contents.
pub(crate) fn replace(
    guard: &WriteLock,
    bytes: &[u8],
    mode: Option<u32>,
) -> io::Result<crate::Committed> {
    let existing = existing_target(&guard.target)?;
    let mode = if existing.is_some() { None } else { mode };
    stage(guard, bytes, mode)?;
    write_target(&guard.target, existing, bytes, mode)?;
    let warning =
        crate::replace::sync_parent_dir(guard.target.parent().unwrap_or_else(|| Path::new(".")))
            .and_then(|()| cleanup(guard))
            .err();
    Ok(crate::Committed { dir_sync: warning })
}

/// Completes a durable pending write before another writer may read or append.
pub(crate) fn replay(guard: &WriteLock) -> io::Result<()> {
    let bytes = match fs::read(&guard.recovery) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return sync_root(recovery_root(guard)?);
        }
        Err(error) => return Err(error),
    };
    if bytes.len() < HEADER_LENGTH || bytes[..32] != guard.digest || bytes[32] > 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "recovery target identity or header mismatch",
        ));
    }
    let mode =
        (bytes[32] == 1).then(|| u32::from_le_bytes([bytes[33], bytes[34], bytes[35], bytes[36]]));
    sync_root(recovery_root(guard)?)?;
    write_target(
        &guard.target,
        existing_target(&guard.target)?,
        &bytes[HEADER_LENGTH..],
        mode,
    )?;
    crate::replace::sync_parent_dir(guard.target.parent().unwrap_or_else(|| Path::new(".")))?;
    cleanup(guard)
}

/// Opens an existing writable target without truncating or following a replaced link.
fn existing_target(path: &Path) -> io::Result<Option<File>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Ok(None),
        Ok(_) => OpenOptions::new().write(true).open(path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Stages a private identity-prefixed copy, then durably publishes its recovery name.
fn stage(guard: &WriteLock, bytes: &[u8], mode: Option<u32>) -> io::Result<()> {
    let root = recovery_root(guard)?;
    let temporary = root.join(format!(
        ".maestro-{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    #[cfg(test)]
    crate::replace::tests::before_create(&temporary)?;
    let mut open = OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    open.mode(0o600);
    let file = open.open(&temporary)?;
    let result = stage_bytes(file, guard, bytes, mode).and_then(|()| {
        #[cfg(test)]
        crate::replace::tests::closed();
        #[cfg(test)]
        crate::replace::tests::effect("rename")?;
        fs::rename(&temporary, &guard.recovery)
    });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    sync_root(root)
}

/// Writes and syncs the complete recovery record before its handle closes.
fn stage_bytes(
    mut file: File,
    guard: &WriteLock,
    bytes: &[u8],
    mode: Option<u32>,
) -> io::Result<()> {
    #[cfg(test)]
    crate::replace::tests::effect("write_all")?;
    file.write_all(&guard.digest)?;
    file.write_all(&[u8::from(mode.is_some())])?;
    file.write_all(&mode.unwrap_or_default().to_le_bytes())?;
    file.write_all(bytes)?;
    #[cfg(test)]
    crate::replace::tests::effect("file_sync")?;
    file.sync_all()
}

/// Writes through the existing inode, creating only a previously absent target.
fn write_target(
    path: &Path,
    existing: Option<File>,
    bytes: &[u8],
    mode: Option<u32>,
) -> io::Result<()> {
    #[cfg(test)]
    crate::replace::tests::effect("target_write")?;
    let mut file = match existing {
        Some(file) => file,
        None => create_target(path, mode)?,
    };
    file.set_len(0)?;
    file.write_all(bytes)?;
    #[cfg(test)]
    crate::replace::tests::effect("target_sync")?;
    file.sync_all()?;
    if let Some(mode) = mode {
        apply_creation_mode(&file, mode)?;
        file.sync_all()?;
    }
    Ok(())
}

/// Creates explicit-mode targets privately and default-mode targets with native flags.
fn create_target(path: &Path, mode: Option<u32>) -> io::Result<File> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        fs::remove_file(path)?;
    }
    let mut open = OpenOptions::new();
    open.create_new(true).write(true);
    #[cfg(unix)]
    if mode.is_some() {
        open.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = mode;
    open.open(path)
}

/// Applies a new target's requested mode only after its complete bytes are synced.
fn apply_creation_mode(file: &File, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    #[cfg(windows)]
    {
        let mut permissions = file.metadata()?.permissions();
        permissions.set_readonly(mode & 0o200 == 0);
        file.set_permissions(permissions)?;
    }
    #[cfg(not(any(unix, windows)))]
    let _ = (file, mode);
    Ok(())
}

/// Removes a completed record only after target and parent synchronization.
fn cleanup(guard: &WriteLock) -> io::Result<()> {
    fs::remove_file(&guard.recovery)?;
    sync_root(recovery_root(guard)?)
}

/// Returns the directory holding one target's recovery record.
fn recovery_root(guard: &WriteLock) -> io::Result<&Path> {
    guard
        .recovery
        .parent()
        .ok_or_else(|| io::Error::other("missing recovery root"))
}

/// Persists recovery entries strictly where native directory synchronization is supported.
fn sync_root(root: &Path) -> io::Result<()> {
    #[cfg(test)]
    crate::replace::tests::effect("recovery_sync")?;
    #[cfg(windows)]
    {
        crate::replace::sync_directory(root)
    }
    #[cfg(not(windows))]
    {
        File::open(root)?.sync_all()
    }
}
