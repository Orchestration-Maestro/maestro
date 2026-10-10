//! Native credential file guarded by a sidecar lock file.
use super::{AuthStorageBackend, AuthStorageError, LockUpdate};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::time::Duration;

/// Attempts made on a contended lock.
const LOCK_ATTEMPTS: u32 = 10;
/// Wait between two contended attempts.
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(20);

/// Credential file at a caller-resolved path; relative paths use the working directory.
#[derive(Debug)]
pub struct FileAuthStorageBackend {
    /// Credential file location.
    auth_path: String,
}

impl FileAuthStorageBackend {
    /// Address the credential file; nothing is created until first use.
    #[must_use]
    pub fn new(auth_path: &str) -> Self {
        Self {
            auth_path: auth_path.to_owned(),
        }
    }

    /// Create missing parents privately and open the sidecar lock file.
    fn lock_file(&self) -> io::Result<File> {
        if let Some(parent) = Path::new(&self.auth_path).parent() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            builder.create(parent)?;
        }
        let sidecar = format!("{}.lock", self.auth_path);
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(sidecar)
    }

    /// Lock the sidecar, retrying only while another holder has it.
    fn acquire(&self) -> Result<File, AuthStorageError> {
        let file = self.lock_file()?;
        retry_contended(|| file.try_lock())?;
        Ok(file)
    }

    /// Initialize a missing file (following a symlink), then read it, under the held lock.
    fn read(&self) -> io::Result<Option<String>> {
        let path = Path::new(&self.auth_path);
        initialize(path)?;
        match fs::read(path) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// Try up to `LOCK_ATTEMPTS` times, waiting only after a contended attempt.
fn retry_contended(
    mut try_lock: impl FnMut() -> Result<(), TryLockError>,
) -> Result<(), AuthStorageError> {
    for attempt in 1..=LOCK_ATTEMPTS {
        match try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if attempt < LOCK_ATTEMPTS => {
                std::thread::sleep(LOCK_RETRY_DELAY);
            }
            Err(error) => return Err(Box::new(error)),
        }
    }
    Err(Box::new(TryLockError::WouldBlock))
}

/// Create the file with `{}` only if absent, following dangling links; an existing file is never opened for writing.
fn initialize(path: &Path) -> io::Result<()> {
    let mut candidate = path.to_path_buf();
    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                io::Write::write_all(&mut file, b"{}")?;
                #[cfg(unix)]
                file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
                return Ok(());
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                match fs::metadata(&candidate) {
                    Err(missing) if missing.kind() == ErrorKind::NotFound => {
                        let link = fs::read_link(&candidate)?;
                        candidate = candidate.parent().unwrap_or(Path::new("")).join(link);
                    }
                    Err(other) => return Err(other),
                    Ok(_) => return Ok(()),
                }
            }
            Err(error) => return Err(error),
        }
    }
}

/// Replace the text, restricting it to its owner on Unix.
fn write(path: &Path, text: &str) -> io::Result<()> {
    fs::write(path, text)?;
    #[cfg(unix)]
    fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(())
}

impl AuthStorageBackend for FileAuthStorageBackend {
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
    ) -> Result<(), AuthStorageError> {
        let lock = self.acquire()?;
        let outcome = self
            .read()
            .map_err(AuthStorageError::from)
            .and_then(|current| update(current.as_deref()))
            .and_then(|next| match next {
                Some(next) => Ok(write(Path::new(&self.auth_path), &next)?),
                None => Ok(()),
            });
        lock.unlock()?;
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_contention_errors_are_not_retried() {
        let mut calls = 0;
        let result = retry_contended(|| {
            calls += 1;
            Err(TryLockError::Error(io::Error::other("denied")))
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn contention_is_retried_until_the_lock_is_free() {
        let mut calls = 0;
        let result = retry_contended(|| {
            calls += 1;
            if calls < 3 {
                Err(TryLockError::WouldBlock)
            } else {
                Ok(())
            }
        });
        assert!(result.is_ok());
        assert_eq!(calls, 3);
    }
}
