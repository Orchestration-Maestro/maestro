//! Durable byte publication through a temporary sibling.
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Symbolic-link policy selected by the calling feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct Replace {
    /// Follow existing symbolic links, including dangling final targets.
    pub through_symlinks: bool,
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
/// Returns native errors before publication, or an error for an unresolved link
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
    let mut name = std::ffi::OsString::from(".");
    name.push(
        target
            .file_name()
            .ok_or_else(|| io::Error::other("replacement target has no file name"))?,
    );
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let temp = parent.join(name);
    #[cfg(test)]
    tests::before_create(&temp)?;
    let file = File::create_new(&temp)?;
    if let Err(error) = write_and_sync(file, bytes) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    #[cfg(test)]
    tests::closed();
    if let Err(error) = commit(&temp, &target) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(Committed {
        dir_sync: sync_parent_dir(parent).err(),
    })
}
/// Writes all bytes before syncing the temporary file.
fn write_and_sync(mut file: File, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    tests::effect("write_all")?;
    file.write_all(bytes)?;
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
