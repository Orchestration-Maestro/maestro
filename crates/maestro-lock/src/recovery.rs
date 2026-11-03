//! Durable roll-forward bytes for an exclusively held canonical target.
use crate::{CreationMode, WriteLock};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

/// Permission-policy tag and mode preceding the intended target bytes.
const HEADER_LENGTH: usize = 5;

/// Publishes intended bytes before touching the original file's identity or contents.
pub(crate) fn replace(
    guard: &WriteLock,
    bytes: &[u8],
    mode: Option<CreationMode>,
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

/// Persists recovery absence before appending bytes through the retained holder.
pub(crate) fn append(guard: &WriteLock, bytes: &[u8]) -> io::Result<()> {
    sync_root(recovery_root(guard)?)?;
    #[cfg(test)]
    crate::replace::tests::effect("target_append")?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&guard.target)?
        .write_all(bytes)
}

/// Completes a durable pending write before another writer may read or append.
pub(crate) fn replay(guard: &WriteLock) -> io::Result<()> {
    let bytes = match fs::read(&guard.recovery) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if bytes.len() < HEADER_LENGTH || bytes[0] > 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid recovery header",
        ));
    }
    let bits = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
    let mode = match bytes[0] {
        1 => Some(CreationMode::AfterSync(bits)),
        2 => Some(CreationMode::AtCreation(bits)),
        _ => None,
    };
    sync_root(recovery_root(guard)?)?;
    let intended = &bytes[HEADER_LENGTH..];
    if !finish_creation(&guard.target, intended, mode)? {
        #[cfg(unix)]
        let mode = prepare_readonly_replay(guard, intended, mode)?;
        write_target(
            &guard.target,
            existing_target(&guard.target)?,
            intended,
            mode,
        )?;
    }
    crate::replace::sync_parent_dir(guard.target.parent().unwrap_or_else(|| Path::new(".")))?;
    cleanup(guard)
}

/// Durably retains observed permissions before granting owner-write for read-only replay.
#[cfg(unix)]
fn prepare_readonly_replay(
    guard: &WriteLock,
    bytes: &[u8],
    mode: Option<CreationMode>,
) -> io::Result<Option<CreationMode>> {
    let Some(mode) = mode else { return Ok(None) };
    let metadata = match fs::symlink_metadata(&guard.target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Some(mode)),
        result => result?,
    };
    let observed = metadata.permissions().mode();
    if !metadata.is_file() || observed & 0o200 != 0 {
        return Ok(Some(mode));
    }
    let retained = match mode {
        CreationMode::AtCreation(_) => {
            let retained = CreationMode::AfterSync(observed);
            stage(guard, bytes, Some(retained))?;
            retained
        }
        CreationMode::AfterSync(_) => mode,
    };
    #[cfg(test)]
    crate::replace::tests::effect("replay_chmod")?;
    fs::set_permissions(&guard.target, fs::Permissions::from_mode(observed | 0o200))?;
    #[cfg(test)]
    crate::replace::tests::effect("replay_widened")?;
    Ok(Some(retained))
}

/// Recognizes a completed read-only creation without requiring another writable open.
fn finish_creation(path: &Path, bytes: &[u8], mode: Option<CreationMode>) -> io::Result<bool> {
    let Some(mode) = mode else {
        return Ok(false);
    };
    if !fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().readonly())
    {
        return Ok(false);
    }
    let mut file = match File::open(path) {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => return Ok(false),
        result => result?,
    };
    let mut current = Vec::new();
    file.read_to_end(&mut current)?;
    if current != bytes {
        return Ok(false);
    }
    let permissions = file.metadata()?.permissions();
    #[cfg(unix)]
    let matches = match mode {
        CreationMode::AfterSync(bits) => permissions.mode() & 0o7777 == bits & 0o7777,
        CreationMode::AtCreation(bits) => permissions.mode() & 0o7777 & !bits == 0,
    };
    #[cfg(windows)]
    let matches = permissions.readonly() == (mode.bits() & 0o200 == 0);
    #[cfg(not(any(unix, windows)))]
    let matches = {
        let _ = (permissions, mode);
        true
    };
    if matches {
        sync_creation(path, &file)?;
    }
    Ok(matches)
}

/// Persists verified creation metadata before retiring its recovery record.
fn sync_creation(path: &Path, file: &File) -> io::Result<()> {
    #[cfg(windows)]
    {
        let permissions = file.metadata()?.permissions();
        let mut writable = permissions.clone();
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "clears only the Windows read-only attribute on a verified recovered creation"
        )]
        writable.set_readonly(false);
        fs::set_permissions(path, writable)?;
        let result = (|| {
            let writable = OpenOptions::new().write(true).open(path)?;
            writable.sync_all()?;
            writable.set_permissions(permissions.clone())?;
            sync_creation_mode(&writable)
        })();
        if result.is_err() {
            fs::set_permissions(path, permissions)?;
        }
        result
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        sync_creation_mode(file)
    }
}

/// Synchronizes final creation permissions, retaining the interruption boundary.
fn sync_creation_mode(file: &File) -> io::Result<()> {
    #[cfg(test)]
    crate::replace::tests::effect("mode_sync")?;
    file.sync_all()
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

/// Stages a private copy, then durably publishes its recovery name.
fn stage(guard: &WriteLock, bytes: &[u8], mode: Option<CreationMode>) -> io::Result<()> {
    let root = recovery_root(guard)?;
    let temporary = crate::atomic::temporary_path(&guard.recovery, crate::Temporary::Unique);
    #[cfg(test)]
    crate::replace::tests::before_create(&temporary)?;
    let mut open = OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    open.mode(0o600);
    let file = open.open(&temporary)?;
    let result = stage_bytes(file, bytes, mode).and_then(|()| {
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
fn stage_bytes(mut file: File, bytes: &[u8], mode: Option<CreationMode>) -> io::Result<()> {
    #[cfg(test)]
    crate::replace::tests::effect("write_all")?;
    let tag = match mode {
        None => 0,
        Some(CreationMode::AfterSync(_)) => 1,
        Some(CreationMode::AtCreation(_)) => 2,
    };
    file.write_all(&[tag])?;
    file.write_all(&mode.map_or(0, CreationMode::bits).to_le_bytes())?;
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
    mode: Option<CreationMode>,
) -> io::Result<()> {
    #[cfg(test)]
    crate::replace::tests::effect("target_write")?;
    let mut file = match existing {
        Some(file) => file,
        None => create_target(path, mode)?,
    };
    #[cfg(test)]
    crate::replace::tests::created();
    file.set_len(0)?;
    #[cfg(test)]
    crate::replace::tests::effect("target_truncated")?;
    file.write_all(bytes)?;
    #[cfg(test)]
    crate::replace::tests::effect("target_sync")?;
    file.sync_all()?;
    if let Some(mode) =
        mode.filter(|mode| matches!(mode, CreationMode::AfterSync(_)) || !cfg!(unix))
    {
        apply_creation_mode(&file, mode.bits())?;
        sync_creation_mode(&file)?;
    }
    Ok(())
}

/// Creates a target with open-time bits, private staging bits or native defaults.
fn create_target(path: &Path, mode: Option<CreationMode>) -> io::Result<File> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        fs::remove_file(path)?;
    }
    let mut open = OpenOptions::new();
    open.create_new(true).write(true);
    #[cfg(unix)]
    match mode {
        Some(CreationMode::AtCreation(bits)) => {
            open.mode(bits);
        }
        Some(CreationMode::AfterSync(_)) => {
            open.mode(0o600);
        }
        None => (),
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
