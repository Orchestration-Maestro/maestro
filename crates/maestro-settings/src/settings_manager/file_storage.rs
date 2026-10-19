//! Native file storage with sidecar file locks.

use std::fs::{self, File, OpenOptions};
use std::path::Path;

use maestro_path::join;

use super::preferences::{SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate};

/// Storage backed by `settings.json` files at caller-supplied locations.
#[derive(Debug)]
pub struct FileSettingsStorage {
    /// The authored location of the global preference file.
    global: String,
    /// The authored location of the project preference file.
    project: String,
}

impl FileSettingsStorage {
    /// Addresses `settings.json` in `agent_dir` and in `config_dir` below `cwd`.
    /// Each location is joined by the shared path functions, giving the runtime's
    /// join results.
    #[must_use]
    pub fn new(cwd: &str, agent_dir: &str, config_dir: &str) -> Self {
        Self {
            global: join(&[agent_dir, "settings.json"]),
            project: join(&[cwd, config_dir, "settings.json"]),
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
        Ok(Self(maestro_lock::acquire(file)?))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Closing the handle releases the lock even when this unlock fails.
        let _ = self.0.unlock();
    }
}

impl FileSettingsStorage {
    /// Runs the shared raw-text operation with the requested write intent.
    fn transact(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
        write: bool,
    ) -> Result<(), SettingsStorageError> {
        let path = Path::new(match scope {
            SettingsScope::Global => &self.global,
            SettingsScope::Project => &self.project,
        });
        if write {
            create_parent(path)?;
        }
        let mut lock = (write || path.exists())
            .then(|| Lock::acquire(path))
            .transpose()?;
        let current = if lock.is_some() {
            match fs::read_to_string(path) {
                Ok(text) => Some(text),
                Err(error) if write && error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            }
        } else {
            None
        };
        let Some(next) = update(current.as_deref())? else {
            return Ok(());
        };
        create_parent(path)?;
        if lock.is_none() {
            lock = Some(Lock::acquire(path)?);
        }
        fs::write(path, next)?;
        drop(lock);
        Ok(())
    }
}

/// Creates a missing settings parent directory without creating the settings file.
fn create_parent(path: &Path) -> Result<(), SettingsStorageError> {
    if let Some(directory) = path.parent().filter(|directory| !directory.exists()) {
        fs::create_dir_all(directory)?;
    }
    Ok(())
}

impl SettingsStorage for FileSettingsStorage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.transact(scope, update, false)
    }

    fn with_write_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.transact(scope, update, true)
    }
}
