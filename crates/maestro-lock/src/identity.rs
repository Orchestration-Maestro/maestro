//! Canonical native write targets and per-user exclusion identities.
use crate::{WriteLock, acquire_waiting};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    fmt::Write,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Whether this process has persisted the bounded private state ancestry.
static ROOT_SYNCED: Mutex<bool> = Mutex::new(false);

/// Acquires canonical file-write exclusion and completes any interrupted publication.
///
/// Missing paths use their deepest existing canonical ancestor. The caller
/// selects whether to follow final symbolic links; authored paths are never modified.
///
/// # Errors
/// Returns native resolution, lock or recovery errors before admitting the writer.
pub fn acquire_write(path: &Path, through_symlinks: bool) -> io::Result<WriteLock> {
    let target = canonical_target(path, through_symlinks)?;
    let digest: [u8; 32] = Sha256::digest(target.as_os_str().as_encoded_bytes()).into();
    let mut name = String::with_capacity(64);
    for byte in digest {
        write!(name, "{byte:02x}").map_err(io::Error::other)?;
    }
    let root = lock_root()?;
    let mut open = fs::OpenOptions::new();
    open.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    open.mode(0o600);
    let file = acquire_waiting(open.open(root.join(format!("{name}.lock")))?)?;
    let guard = WriteLock {
        target,
        recovery: root.join(format!("{name}.recovery")),
        digest,
        _file: file,
    };
    crate::recovery::replay(&guard)?;
    Ok(guard)
}

/// Creates a private state directory independent of configuration-root overrides.
fn lock_root() -> io::Result<PathBuf> {
    #[cfg(test)]
    let root = crate::replace::tests::root();
    #[cfg(not(test))]
    let root = maestro_path::native_homedir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))?
        .join(".maestro/state/file-locks");
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(&root)?;
    let mut synced = ROOT_SYNCED
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?;
    if !*synced {
        #[cfg(test)]
        crate::replace::tests::effect("ancestry_sync")?;
        for directory in root.ancestors().take(4) {
            #[cfg(test)]
            crate::replace::tests::ancestry_directory(directory);
            #[cfg(windows)]
            crate::replace::sync_directory(directory)?;
            #[cfg(not(windows))]
            fs::File::open(directory)?.sync_all()?;
        }
        *synced = true;
    }
    Ok(root)
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
