//! Native credential file guarded by a sidecar lock file.
use super::{AuthStorageBackend, AuthStorageError, LockUpdate};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Attempts made on a contended lock.
const LOCK_ATTEMPTS: u32 = 10;
/// Wait between two contended attempts.
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(20);

/// Credential file at a caller-resolved path; relative paths use the working directory.
#[derive(Debug)]
pub struct FileAuthStorageBackend {
    /// Credential file location.
    auth_path: PathBuf,
}

impl FileAuthStorageBackend {
    /// Address the credential file; nothing is created until first use.
    #[must_use]
    pub fn new(auth_path: &str) -> Self {
        Self {
            auth_path: PathBuf::from(auth_path),
        }
    }

    /// Create missing parents privately and open the sidecar lock file.
    fn lock_file(&self) -> io::Result<File> {
        if let Some(parent) = self.auth_path.parent() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            builder.create(parent)?;
        }
        let mut sidecar = self.auth_path.clone().into_os_string();
        sidecar.push(".lock");
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(sidecar)
    }

    /// Lock the sidecar, retrying only while another holder has it.
    fn acquire(&self) -> Result<File, AuthStorageError> {
        let file = self.lock_file()?;
        for attempt in 1..=LOCK_ATTEMPTS {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(TryLockError::WouldBlock) if attempt < LOCK_ATTEMPTS => {
                    std::thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(Box::new(error)),
            }
        }
        Err(Box::new(TryLockError::WouldBlock))
    }

    /// Initialize a missing file, then read it, under the held lock.
    fn read(&self) -> io::Result<Option<String>> {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.auth_path)
        {
            Ok(_) => write(&self.auth_path, "{}")?,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        match fs::read(&self.auth_path) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
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
                Some(next) => Ok(write(&self.auth_path, &next)?),
                None => Ok(()),
            });
        lock.unlock()?;
        outcome
    }
}
