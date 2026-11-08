//! Recoverable byte publication preserving native file identity.
#[cfg(all(test, unix))]
use crate::identity::resolve_write_target;
use std::{fs::File, io, path::Path};

/// Permission timing for a newly created target; existing access controls always win.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationMode {
    /// Apply Unix bits at open, filtered by umask, without a later chmod.
    /// Other platforms retain native creation and owner-write/read-only mapping.
    AtCreation(u32),
    /// Create privately, then apply exact bits after syncing the complete bytes.
    AfterSync(u32),
}

impl CreationMode {
    /// Returns the native permission bits without changing their timing policy.
    pub(crate) const fn bits(self) -> u32 {
        match self {
            Self::AtCreation(bits) | Self::AfterSync(bits) => bits,
        }
    }
}

/// Symbolic-link and creation policy selected by the calling feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct Replace {
    /// Follow final symbolic links, including dangling targets.
    pub through_symlinks: bool,
    /// Creation policy for missing files; existing access controls remain unchanged.
    pub creation_mode: Option<CreationMode>,
}
/// Successful publication with an optional postcommit durability warning.
#[derive(Debug)]
pub struct Committed {
    /// Unexpected directory sync or recovery cleanup error after target sync.
    pub dir_sync: Option<io::Error>,
}
/// Narrows the existing target at `path` to `bits`, leaving a missing file missing.
///
/// The caller chooses the mode it publishes under. An existing file must be writable by its
/// owner, so the handle that opens it proves that before its bits narrow; a read-only file
/// fails with the native permission error and keeps its bytes.
///
/// # Errors
/// Returns the native open or permission error of an existing file it cannot narrow.
#[cfg(unix)]
pub fn narrow_existing(path: &Path, bits: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    match File::options().write(true).open(path) {
        Ok(file) => file.set_permissions(std::fs::Permissions::from_mode(bits)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Publishes intended bytes durably, then writes the target in place under exclusion.
///
/// No directory is created: a missing parent, including the missing target directory of a
/// dangling link, fails with the native not-found error and leaves the link in place.
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
