use crate::{
    SettingsError, SettingsFileError, SettingsLocations, SettingsScope, SettingsStorage,
    SettingsTransaction,
};
use serde_json::{Map, Value};
use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

/// JSON-object storage with cooperative native exclusion for scoped transactions.
/// Reads take no lock and create nothing. Transactions retain a persistent
/// `settings.json.lock` sidecar and hold an independent owned file handle through
/// parsing, editing, serialization and in-place writing. External editors and
/// path aliases need not cooperate. In-place I/O failures can leave partial bytes;
/// lock-free readers can observe a partial write. No crash durability or rollback
/// is promised.
pub struct FileSettingsStorage {
    locations: SettingsLocations,
    cancelled: Arc<AtomicBool>,
    #[cfg(test)]
    controls: Controls,
}
impl FileSettingsStorage {
    /// Constructs without I/O using explicit locations and caller-owned cancellation.
    /// A true signal cancels unadmitted transactions and is never reset here.
    /// Cancellation after admission does not interrupt or undo the transaction.
    pub fn new(locations: SettingsLocations, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            locations,
            cancelled,
            #[cfg(test)]
            controls: Controls::default(),
        }
    }
    fn path(&self, scope: SettingsScope) -> PathBuf {
        self.locations
            .configuration_directory(scope)
            .join("settings.json")
    }
    fn error(&self, scope: SettingsScope, kind: SettingsFileError) -> SettingsError {
        SettingsError::File {
            scope,
            path: self.path(scope),
            kind,
        }
    }
    fn check_cancelled(&self, scope: SettingsScope) -> Result<(), SettingsError> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(self.error(scope, SettingsFileError::Cancelled))
        } else {
            Ok(())
        }
    }
    fn acquire(&mut self, file: &File) -> Result<(), TryLockError> {
        #[cfg(test)]
        if let Some(acquire) = &mut self.controls.acquire {
            return acquire(file);
        }
        file.try_lock()
    }
    fn wait(&mut self, duration: Duration) {
        #[cfg(test)]
        if let Some(wait) = &mut self.controls.wait {
            wait(duration);
            return;
        }
        std::thread::sleep(duration);
    }
    #[cfg(test)]
    fn failure(&mut self, point: FailurePoint, scope: SettingsScope) -> Result<(), SettingsError> {
        if let Some(failure) = &mut self.controls.failure {
            failure(point).map_err(|e| self.error(scope, SettingsFileError::Io(e.kind())))?;
        }
        Ok(())
    }
}
impl SettingsStorage for FileSettingsStorage {
    fn read(&mut self, scope: SettingsScope) -> Result<Map<String, Value>, SettingsError> {
        let bytes = match std::fs::read(self.path(scope)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
            Err(e) => return Err(self.error(scope, SettingsFileError::Io(e.kind()))),
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| {
            self.error(
                scope,
                SettingsFileError::Malformed {
                    line: e.line(),
                    column: e.column(),
                },
            )
        })?;
        match value {
            Value::Object(map) => Ok(map),
            _ => Err(self.error(scope, SettingsFileError::NotObject)),
        }
    }
    fn transact(
        &mut self,
        scope: SettingsScope,
        edit: &mut SettingsTransaction<'_>,
    ) -> Result<(), SettingsError> {
        self.check_cancelled(scope)?;
        let path = self.path(scope);
        let dir = self.locations.configuration_directory(scope);
        std::fs::create_dir_all(&dir)
            .map_err(|e| self.error(scope, SettingsFileError::Io(e.kind())))?;
        // The owned handle releases only its own lock on every return or unwind.
        // Never unlink the sidecar: its identity must outlive an individual writer.
        let guard = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("settings.json.lock"))
            .map_err(|e| self.error(scope, SettingsFileError::Io(e.kind())))?;
        for attempt in 0..10 {
            self.check_cancelled(scope)?;
            match self.acquire(&guard) {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) if attempt < 9 => {
                    self.wait(Duration::from_millis(20))
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(self.error(scope, SettingsFileError::Contended));
                }
                Err(TryLockError::Error(e)) => {
                    return Err(self.error(scope, SettingsFileError::Io(e.kind())));
                }
            }
        }
        self.check_cancelled(scope)?;
        #[cfg(test)]
        self.failure(FailurePoint::Read, scope)?;
        let current = self.read(scope)?;
        self.check_cancelled(scope)?;
        if let Some(next) = edit(&current)? {
            // Serialize before truncating; logical failures leave settings bytes intact.
            let bytes = serde_json::to_vec_pretty(&next).map_err(|e| {
                self.error(
                    scope,
                    SettingsFileError::Io(
                        e.io_error_kind().unwrap_or(std::io::ErrorKind::InvalidData),
                    ),
                )
            })?;
            #[cfg(test)]
            self.failure(FailurePoint::BeforeWrite, scope)?;
            let mut destination = File::create(path)
                .map_err(|e| self.error(scope, SettingsFileError::Io(e.kind())))?;
            #[cfg(test)]
            self.failure(FailurePoint::AfterTruncate, scope)?;
            destination
                .write_all(&bytes)
                .map_err(|e| self.error(scope, SettingsFileError::Io(e.kind())))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq)]
enum FailurePoint {
    Read,
    BeforeWrite,
    AfterTruncate,
}
#[cfg(test)]
type Acquisition = dyn FnMut(&File) -> Result<(), TryLockError> + Send;
#[cfg(test)]
type Failure = dyn FnMut(FailurePoint) -> Result<(), std::io::Error> + Send;
#[cfg(test)]
#[derive(Default)]
struct Controls {
    acquire: Option<Box<Acquisition>>,
    wait: Option<Box<dyn FnMut(Duration) + Send>>,
    failure: Option<Box<Failure>>,
}
#[cfg(test)]
mod tests;
