//! Native file policy; publication mechanics remain with the lock owner.
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::{fs, io, path::Path};

/// Publishes authored bytes without replacing a final symbolic link.
pub(super) fn save_file(path: &str, bytes: &[u8], mode: u32) -> io::Result<Option<String>> {
    maestro_lock::replace(
        Path::new(path),
        bytes,
        maestro_lock::Replace {
            through_symlinks: true,
            creation_mode: Some(mode),
        },
    )
    .map(|committed| committed.dir_sync.map(|error| error.to_string()))
}

/// Reads the native permission representation rather than synthesizing POSIX bits.
pub(super) fn file_mode(path: &str) -> io::Result<u32> {
    let permissions = fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        Ok(permissions.mode())
    }
    #[cfg(not(unix))]
    {
        let _ = permissions;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Applies native Unix permissions to the followed path.
pub(super) fn set_file_mode(path: &str, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Requests creation bits before any new Unix directory becomes visible.
pub(super) fn create_dir(path: &str, mode: u32) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(mode);
    #[cfg(not(unix))]
    let _ = mode;
    builder.create(path)
}
