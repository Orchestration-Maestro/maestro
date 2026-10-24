//! Private temporary files that never replace an existing entry.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

/// Creates `path` exclusively, readable only by its owner where the platform has modes,
/// and writes `bytes`; after a failed write, removal of the file it created is attempted
/// and a failed removal ignored.
///
/// # Errors
/// Returns the creation failure, including an existing entry or symbolic link at `path`,
/// or the write failure.
pub(crate) fn write_temporary_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let result = file.write_all(bytes);
    drop(file);
    if result.is_err() {
        // The write failure is the one reported; the file is ours to remove either way.
        let _ = fs::remove_file(path);
    }
    result
}
