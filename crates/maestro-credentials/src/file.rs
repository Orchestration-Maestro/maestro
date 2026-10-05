//! Explicit-path private file transactions on stable native file identity.
use crate::{CredentialError, CredentialStorage, CredentialTransaction};
use maestro_models::{Cancellation, SecretString};
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{Read, Seek, Write},
    path::PathBuf,
    time::Duration,
};

/// Private plaintext credential file, protected only against cooperating writers.
/// Uses independent handles and owned native guards; no rename or crash rollback.
pub struct FileCredentialStorage {
    path: PathBuf,
    acquisition: std::sync::Arc<dyn Acquisition>,
}
trait Acquisition: Send + Sync {
    fn try_lock(&self, file: &File) -> Result<(), TryLockError>;
    fn wait(&self, duration: Duration, cancellation: &Cancellation);
}
struct NativeAcquisition;
impl Acquisition for NativeAcquisition {
    fn try_lock(&self, file: &File) -> Result<(), TryLockError> {
        file.try_lock()
    }
    fn wait(&self, duration: Duration, _: &Cancellation) {
        std::thread::sleep(duration);
    }
}
impl FileCredentialStorage {
    /// Bind an absolute file path without I/O or ambient installation discovery.
    pub fn new(path: PathBuf) -> Result<Self, CredentialError> {
        if !path.is_absolute() || path.file_name().is_none() {
            return Err(CredentialError::InvalidPath);
        }
        Ok(Self {
            path,
            acquisition: std::sync::Arc::new(NativeAcquisition),
        })
    }
}
struct Guard(File);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
impl CredentialStorage for FileCredentialStorage {
    fn transact(
        &self,
        cancellation: &Cancellation,
        edit: &mut CredentialTransaction<'_>,
    ) -> Result<(), CredentialError> {
        if cancellation.is_cancelled() {
            return Err(CredentialError::Cancelled);
        }
        let mut parents = std::fs::DirBuilder::new();
        parents.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            parents.mode(0o700);
        }
        parents
            .create(self.path.parent().ok_or(CredentialError::InvalidPath)?)
            .map_err(|_| CredentialError::Storage)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let (file, created) = match options.open(&self.path) {
            Ok(file) => (file, true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.path)
                    .map_err(|_| CredentialError::Storage)?,
                false,
            ),
            Err(_) => return Err(CredentialError::Storage),
        };
        for attempt in 0..10 {
            if cancellation.is_cancelled() {
                return Err(CredentialError::Cancelled);
            }
            match self.acquisition.try_lock(&file) {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) if attempt < 9 => self
                    .acquisition
                    .wait(Duration::from_millis(20), cancellation),
                Err(TryLockError::WouldBlock) => return Err(CredentialError::Contended),
                Err(TryLockError::Error(_)) => return Err(CredentialError::Storage),
            }
        }
        let mut guard = Guard(file);
        if cancellation.is_cancelled() {
            return Err(CredentialError::Cancelled);
        }
        let mut bytes = String::new();
        guard.0.read_to_string(&mut bytes).map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                CredentialError::Malformed
            } else {
                CredentialError::Storage
            }
        })?;
        if created && bytes.is_empty() {
            guard
                .0
                .write_all(b"{}")
                .map_err(|_| CredentialError::Storage)?;
            bytes = "{}".into();
        }
        if cancellation.is_cancelled() {
            return Err(CredentialError::Cancelled);
        }
        if let Some(next) = edit(Some(SecretString::new(bytes)))? {
            guard.0.rewind().map_err(|_| CredentialError::Storage)?;
            guard
                .0
                .write_all(next.expose().as_bytes())
                .map_err(|_| CredentialError::Storage)?;
            guard
                .0
                .set_len(next.expose().len() as u64)
                .map_err(|_| CredentialError::Storage)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
