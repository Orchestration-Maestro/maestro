//! Native file storage with sidecar file locks.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::paths::join;
use super::preferences::{SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate};

/// How often a contended lock is tried before giving up.
const LOCK_ATTEMPTS: u32 = 10;
/// How long to wait between two attempts on a contended lock.
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(20);

/// Storage backed by `settings.json` files at caller-supplied locations.
#[derive(Debug)]
pub struct FileSettingsStorage {
    /// The global preference file.
    global: PathBuf,
    /// The project preference file.
    project: PathBuf,
}

impl FileSettingsStorage {
    /// Addresses `settings.json` in `agent_dir` and in `config_dir` below `cwd`,
    /// joining the parts as text and folding `.` and `..` segments of the result
    /// before any file access.
    #[must_use]
    pub fn new(cwd: &Path, agent_dir: &Path, config_dir: &OsStr) -> Self {
        let file = OsStr::new("settings.json");
        Self {
            global: join(&[agent_dir.as_os_str(), file]),
            project: join(&[cwd.as_os_str(), config_dir, file]),
        }
    }
}

/// An exclusive lock on a sidecar file, released when dropped.
struct Lock(File);

impl Lock {
    /// Locks the sidecar next to `path`, retrying only while another holder has it.
    fn acquire(path: &Path) -> Result<Self, SettingsStorageError> {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(".lock");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(sidecar)?;
        for attempt in 1..=LOCK_ATTEMPTS {
            match file.try_lock() {
                Ok(()) => return Ok(Self(file)),
                Err(TryLockError::WouldBlock) if attempt < LOCK_ATTEMPTS => {
                    std::thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(Box::new(error)),
            }
        }
        Err(Box::new(TryLockError::WouldBlock))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Closing the handle releases the lock even when this unlock fails.
        let _ = self.0.unlock();
    }
}

impl SettingsStorage for FileSettingsStorage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let path = match scope {
            SettingsScope::Global => &self.global,
            SettingsScope::Project => &self.project,
        };
        let exists = path.exists();
        let mut lock = exists.then(|| Lock::acquire(path)).transpose()?;
        let current = exists.then(|| fs::read_to_string(path)).transpose()?;
        let Some(next) = update(current.as_deref())? else {
            return Ok(());
        };
        if let Some(directory) = path.parent().filter(|directory| !directory.exists()) {
            fs::create_dir_all(directory)?;
        }
        if lock.is_none() {
            lock = Some(Lock::acquire(path)?);
        }
        fs::write(path, next)?;
        drop(lock);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn leading_parents_stay_in_front_of_relative_locations() {
        let storage =
            FileSettingsStorage::new(Path::new("../.."), Path::new(".."), OsStr::new(".maestro"));
        // Node `path.posix.join('..', 'settings.json')` and
        // `join('../..', '.maestro', 'settings.json')`.
        assert_eq!(storage.global.as_os_str(), "../settings.json");
        assert_eq!(storage.project.as_os_str(), "../../.maestro/settings.json");
    }

    #[cfg(windows)]
    #[test]
    fn parents_of_a_drive_relative_location_stay_after_the_drive() {
        let storage =
            FileSettingsStorage::new(Path::new("C:.."), Path::new("C:.."), OsStr::new(".maestro"));
        // Node `path.win32.join('C:..', 'settings.json')` and
        // `join('C:..', '.maestro', 'settings.json')`.
        assert_eq!(storage.global.as_os_str(), r"C:..\settings.json");
        assert_eq!(storage.project.as_os_str(), r"C:..\.maestro\settings.json");
    }
}
