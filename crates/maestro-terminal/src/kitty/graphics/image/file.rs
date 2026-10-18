//! Native file admission, bounded reads and temporary-file cleanup.
use super::super::command::{Medium, Transmission};
use super::{ImageError, MAX_SIZE};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// Remove an admitted temporary file on every exit from its read operation.
struct RemoveFile<'a>(&'a Path);
impl Drop for RemoveFile<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0);
    }
}

/// Read a file payload using roots observed for this operation.
pub(super) fn read(bytes: &[u8], transmission: &Transmission) -> Result<Vec<u8>, ImageError> {
    let path = native_path(bytes)?;
    let path = fs::canonicalize(path).map_err(|_| ImageError::InvalidData)?;
    let roots = if transmission.medium == Medium::TemporaryFile {
        temp_dirs()
    } else {
        Vec::new()
    };
    read_path(&path, transmission, &roots)
}

/// Check canonical admission before reading or scheduling cleanup.
pub(super) fn read_path(
    path: &Path,
    transmission: &Transmission,
    roots: &[PathBuf],
) -> Result<Vec<u8>, ImageError> {
    if path.starts_with("/proc")
        || path.starts_with("/sys")
        || (path.starts_with("/dev") && !path.starts_with("/dev/shm"))
    {
        return Err(ImageError::InvalidData);
    }
    let _cleanup = if transmission.medium == Medium::TemporaryFile {
        if !is_path_in_temp_dir(path, roots) {
            return Err(ImageError::TemporaryFileNotInTempDir);
        }
        if !path
            .as_os_str()
            .as_encoded_bytes()
            .windows(b"tty-graphics-protocol".len())
            .any(|part| part == b"tty-graphics-protocol")
        {
            return Err(ImageError::TemporaryFileNotNamedCorrectly);
        }
        Some(RemoveFile(path))
    } else {
        None
    };
    let mut file = File::open(path).map_err(|_| ImageError::InvalidData)?;
    if !file
        .metadata()
        .map_err(|_| ImageError::InvalidData)?
        .is_file()
    {
        return Err(ImageError::InvalidData);
    }
    file.seek(SeekFrom::Start(u64::from(transmission.offset)))
        .map_err(|_| ImageError::InvalidData)?;
    let limit = if transmission.size == 0 {
        MAX_SIZE
    } else {
        u64::from(transmission.size).min(MAX_SIZE)
    };
    read_bounded(file, limit).map_err(|_| ImageError::InvalidData)
}

/// Both operands are canonical paths, compared by components rather than bytes.
pub(super) fn is_path_in_temp_dir(path: &Path, dirs: &[PathBuf]) -> bool {
    dirs.iter().any(|dir| path.starts_with(dir))
}

/// Capture and canonicalize the platform temporary roots once per file load.
fn temp_dirs() -> Vec<PathBuf> {
    [
        PathBuf::from("/tmp"),
        PathBuf::from("/dev/shm"),
        selected_temp_dir(std::env::var_os("TMPDIR"), std::env::var_os("TMP")),
    ]
    .into_iter()
    .filter_map(|path| fs::canonicalize(path).ok())
    .collect()
}

/// Select the first present Unix temporary variable without trimming whitespace.
pub(super) fn selected_temp_dir(
    tmpdir: Option<std::ffi::OsString>,
    tmp: Option<std::ffi::OsString>,
) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let selected = tmpdir.or(tmp).unwrap_or_else(|| "/tmp".into());
        let bytes = selected.as_bytes();
        let end = bytes
            .iter()
            .rposition(|&byte| byte != b'/')
            .map_or(0, |index| index + 1);
        PathBuf::from(std::ffi::OsString::from_vec(bytes[..end].to_vec()))
    }
    #[cfg(not(unix))]
    {
        let _ = (tmpdir, tmp);
        std::env::temp_dir()
    }
}

/// Convert native path bytes without lossy Unicode decoding.
fn native_path(bytes: &[u8]) -> Result<PathBuf, ImageError> {
    if bytes.contains(&0) {
        return Err(ImageError::InvalidData);
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes)
            .map(PathBuf::from)
            .map_err(|_| ImageError::InvalidData)
    }
}

/// Read the whole remainder, rejecting overflow rather than silently truncating.
pub(super) fn read_bounded(reader: impl Read, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}
