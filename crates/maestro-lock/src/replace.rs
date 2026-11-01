//! Recoverable byte publication preserving native file identity.
#[cfg(all(test, unix))]
use crate::identity::resolve_write_target;
use std::{
    fs::{self, File},
    io,
    path::Path,
};

/// Symbolic-link and creation policy selected by the calling feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct Replace {
    /// Follow final symbolic links, including dangling targets.
    pub through_symlinks: bool,
    /// Creation bits for missing files; existing access controls remain unchanged.
    pub creation_mode: Option<u32>,
}
/// Successful publication with an optional postcommit durability warning.
#[derive(Debug)]
pub struct Committed {
    /// Unexpected directory sync or recovery cleanup error after target sync.
    pub dir_sync: Option<io::Error>,
}
/// Publishes intended bytes durably, then writes the target in place under exclusion.
///
/// # Errors
/// Returns native resolution, lock, staging or target-write errors. An interrupted
/// write is completed from its recovery record by the next coordinated writer.
pub fn replace(path: &Path, bytes: &[u8], options: Replace) -> io::Result<Committed> {
    let target = if options.through_symlinks {
        crate::identity::resolve_write_target(path)?
    } else {
        path.to_path_buf()
    };
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    crate::acquire_write(&target, false)?.replace(bytes, options.creation_mode)
}

/// Opens and syncs a directory, tolerating hosts without native directory sync.
pub(crate) fn sync_directory(parent: &Path) -> io::Result<()> {
    tolerate_sync_error(File::open(parent).and_then(|file| file.sync_all()))
}

/// Syncs a target parent after successful publication, retaining test observations.
pub(crate) fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    tolerate_sync_error(sync_observed_parent(parent))
}

/// Performs the observed parent-directory synchronization.
fn sync_observed_parent(parent: &Path) -> io::Result<()> {
    #[cfg(test)]
    tests::effect("parent_open")?;
    #[cfg(test)]
    if File::open(parent).is_ok() {
        tests::effect("parent_sync")?;
    }
    sync_directory(parent)
}

/// Retains the native policy for hosts without directory synchronization.
fn tolerate_sync_error(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::Unsupported
                    | io::ErrorKind::InvalidInput
                    | io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(())
        }
        result => result,
    }
}

#[cfg(test)]
pub(crate) mod tests;
