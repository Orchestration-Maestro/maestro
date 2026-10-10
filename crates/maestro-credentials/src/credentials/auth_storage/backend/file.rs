//! Native credential file guarded by a sidecar lock file.
use super::{AsyncLockUpdate, AuthStorageBackend, AuthStorageError, AuthStorageFuture, LockUpdate};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::time::Duration;

/// Waits between contended attempts of the asynchronous acquisition, in milliseconds.
const ASYNC_LOCK_DELAYS_MS: [u64; 10] = [100, 200, 400, 800, 1600, 3200, 6400, 10000, 10000, 10000];

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
        Ok(maestro_lock::acquire(self.lock_file()?)?)
    }

    /// Lock the sidecar, waiting the scheduled delays while another holder has it.
    async fn acquire_async(&self) -> Result<File, AuthStorageError> {
        let file = self.lock_file()?;
        let mut delays = ASYNC_LOCK_DELAYS_MS.into_iter();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(TryLockError::WouldBlock) => match delays.next() {
                    Some(delay) => tokio::time::sleep(Duration::from_millis(delay)).await,
                    None => return Err(Box::new(TryLockError::WouldBlock)),
                },
                Err(error) => return Err(Box::new(error)),
            }
        }
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

    fn with_lock_async<'a>(
        &'a self,
        update: AsyncLockUpdate<'a>,
    ) -> AuthStorageFuture<'a, Result<(), AuthStorageError>> {
        Box::pin(async move {
            let _lock = self.acquire_async().await?;
            let current = self.read()?;
            if let Some(next) = update(current).await? {
                write(Path::new(&self.auth_path), &next)?;
            }
            Ok(())
        })
    }
}
