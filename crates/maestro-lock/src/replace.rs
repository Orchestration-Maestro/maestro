//! Durable byte publication through a temporary sibling.
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Symbolic-link policy selected by the calling feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct Replace {
    /// Follow existing symbolic links, including dangling final targets.
    pub through_symlinks: bool,
    /// Creation bits for a missing file; existing native permissions win.
    ///
    /// Unix applies all permission bits. Windows maps a missing owner-write bit
    /// to read-only and ignores the other bits; this does not configure ACLs.
    pub creation_mode: Option<u32>,
}
/// Successful publication with an optional postcommit durability warning.
#[derive(Debug)]
pub struct Committed {
    /// Unexpected parent-directory sync error after the bytes were committed.
    pub dir_sync: Option<io::Error>,
}
/// Publishes bytes with file sync before rename and directory sync afterwards.
///
/// # Errors
/// Returns native errors before publication, including the refusal to open an existing target
/// for writing, or an error for an unresolved link
/// after forty hops. Postcommit errors are returned in [`Committed::dir_sync`].
pub fn replace(path: &Path, bytes: &[u8], options: Replace) -> io::Result<Committed> {
    let target = if options.through_symlinks {
        resolve_write_target(path)?
    } else {
        path.to_path_buf()
    };
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    if target.file_name().is_none() {
        return Err(io::Error::other("replacement target has no file name"));
    }
    let temp = parent.join(format!(
        ".maestro-{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    #[cfg(test)]
    tests::before_create(&temp)?;
    let permissions = replacement_permissions(&target)?;
    let file = create_temporary(&temp, permissions.as_ref(), options.creation_mode)?;
    let result = write_and_sync(file, bytes, permissions, options.creation_mode).and_then(|()| {
        #[cfg(test)]
        tests::closed();
        commit(&temp, &target)
    });
    if let Err(error) = result {
        remove_temporary(&temp);
        return Err(error);
    }
    Ok(Committed {
        dir_sync: sync_parent_dir(parent).err(),
    })
}
/// Reads the replaced file's permissions without following a replaced link entry.
///
/// A rename needs write access only to the directory, so an existing file is first opened for
/// writing without truncation: a file its owner made read-only is refused, as a write in place is.
fn replacement_permissions(target: &Path) -> io::Result<Option<Permissions>> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if !metadata.file_type().is_symlink() => {
            OpenOptions::new().write(true).open(target)?;
            Ok(Some(metadata.permissions()))
        }
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Creates a sibling no more permissive than the selected Unix permission bits.
fn create_temporary(
    path: &Path,
    permissions: Option<&Permissions>,
    creation_mode: Option<u32>,
) -> io::Result<File> {
    let mut open = OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(mode) = permissions.map(Permissions::mode).or(creation_mode) {
        open.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = (permissions, creation_mode);
    open.open(path)
}

/// Removes only an owned sibling, clearing Windows read-only deletion protection.
fn remove_temporary(path: &Path) {
    #[cfg(windows)]
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "clears the Windows read-only attribute; cfg(windows) excludes Unix world-writable permissions"
        )]
        permissions.set_readonly(false);
        let _ = fs::set_permissions(path, permissions);
    }
    let _ = fs::remove_file(path);
}

/// Writes all bytes before syncing the temporary file.
fn write_and_sync(
    mut file: File,
    bytes: &[u8],
    permissions: Option<Permissions>,
    creation_mode: Option<u32>,
) -> io::Result<()> {
    #[cfg(test)]
    tests::effect("write_all")?;
    file.write_all(bytes)?;
    #[cfg(unix)]
    let permissions = permissions.or_else(|| creation_mode.map(Permissions::from_mode));
    #[cfg(windows)]
    let permissions = match (permissions, creation_mode) {
        (None, Some(mode)) => {
            let mut selected = file.metadata()?.permissions();
            selected.set_readonly(mode & 0o200 == 0);
            Some(selected)
        }
        (permissions, _) => permissions,
    };
    #[cfg(not(any(unix, windows)))]
    let _ = creation_mode;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    #[cfg(test)]
    tests::effect("file_sync")?;
    file.sync_all()
}

/// Commits the closed temporary sibling.
fn commit(temp: &Path, target: &Path) -> io::Result<()> {
    #[cfg(test)]
    tests::effect("rename")?;
    fs::rename(temp, target)
}

/// Opens a directory handle before syncing it.
fn open_and_sync_directory(parent: &Path) -> io::Result<()> {
    #[cfg(test)]
    tests::effect("parent_open")?;
    let dir = File::open(parent)?;
    #[cfg(test)]
    tests::effect("parent_sync")?;
    dir.sync_all()
}

/// Resolves at most forty final-component links without requiring a target.
fn resolve_write_target(path: &Path) -> io::Result<PathBuf> {
    maestro_path::resolve_final_links(path, |current| {
        let Ok(meta) = fs::symlink_metadata(current) else {
            return Ok(None);
        };
        if meta.file_type().is_symlink() {
            fs::read_link(current).map(Some)
        } else {
            Ok(None)
        }
    })
}
/// Syncs the parent while tolerating unsupported directory sync.
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    let result = open_and_sync_directory(parent);
    match result {
        Err(error) if directory_sync_unsupported(&error) => Ok(()),
        result => result,
    }
}
/// Recognizes native platforms that cannot sync directory handles.
fn directory_sync_unsupported(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied
    )
}

#[cfg(test)]
mod tests;
