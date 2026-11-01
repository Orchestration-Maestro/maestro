//! Native file storage with sidecar file locks.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::path::Path;

use maestro_path::join;

use super::BaseSettingsLayer;
use super::preferences::{SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate};

/// Storage backed by `settings.json` files at caller-supplied locations.
#[derive(Debug)]
pub struct FileSettingsStorage {
    /// The authored location of the global preference file.
    global: String,
    /// The authored location of the project preference file.
    project: String,
    /// Configuration root for persisted base records.
    agent_dir: String,
    /// Supplied project root, resolved only when addressing persisted project state.
    cwd: String,
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
            agent_dir: agent_dir.to_owned(),
            cwd: cwd.to_owned(),
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
        path: &Path,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
        write: bool,
    ) -> Result<(), SettingsStorageError> {
        if write {
            create_parent(path)?;
        }
        let mut lock = (write || path.try_exists()?)
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
        let replacement = maestro_lock::Replace {
            through_symlinks: true,
            creation_mode: None,
        };
        maestro_lock::replace(path, next.as_bytes(), replacement)?;
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

impl FileSettingsStorage {
    /// The raw preference location for a scope.
    fn settings_path(&self, scope: SettingsScope) -> &str {
        match scope {
            SettingsScope::Global => &self.global,
            SettingsScope::Project => &self.project,
        }
    }

    /// The global record or the project-root-indexed state document.
    fn base_path(&self, scope: SettingsScope) -> String {
        match scope {
            SettingsScope::Global => join(&[&self.agent_dir, "base-settings.json"]),
            SettingsScope::Project => join(&[&self.agent_dir, "state", "base-settings.json"]),
        }
    }

    /// Replaces one project record while keeping other roots' records intact.
    fn project_base_text(
        &self,
        current: Option<&str>,
        layer: Option<&BaseSettingsLayer>,
    ) -> Result<String, SettingsStorageError> {
        let mut records: BTreeMap<String, BaseSettingsLayer> =
            serde_json::from_str(current.unwrap_or("{}"))?;
        let key = self.project_key()?;
        if let Some(layer) = layer {
            records.insert(key, layer.clone());
        } else {
            records.remove(&key);
        }
        Ok(serde_json::to_string_pretty(&records)?)
    }

    /// Resolves the project state key without following links or creating the project.
    fn project_key(&self) -> Result<String, SettingsStorageError> {
        let absolute = std::path::absolute(&self.cwd)?;
        let normalized = maestro_path::normalize_native(&absolute);
        normalized
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| std::io::Error::other("project root is not valid UTF-8").into())
    }
}

impl SettingsStorage for FileSettingsStorage {
    fn read_base_layer(
        &self,
        scope: SettingsScope,
    ) -> Result<Option<BaseSettingsLayer>, SettingsStorageError> {
        let mut layer = None;
        Self::transact(
            Path::new(&self.base_path(scope)),
            &mut |current| {
                let Some(text) = current else {
                    return Ok(None);
                };
                layer = match scope {
                    SettingsScope::Global => serde_json::from_str(text)?,
                    SettingsScope::Project => {
                        let mut records: BTreeMap<String, BaseSettingsLayer> =
                            serde_json::from_str(text)?;
                        records.remove(&self.project_key()?)
                    }
                };
                Ok(None)
            },
            false,
        )?;
        Ok(layer)
    }

    fn write_base_layer(
        &self,
        scope: SettingsScope,
        layer: Option<&BaseSettingsLayer>,
    ) -> Result<(), SettingsStorageError> {
        Self::transact(
            Path::new(&self.base_path(scope)),
            &mut |current| {
                if current.is_none() && layer.is_none() {
                    return Ok(None);
                }
                let text = match scope {
                    SettingsScope::Global => serde_json::to_string_pretty(&layer)?,
                    SettingsScope::Project => self.project_base_text(current, layer)?,
                };
                Ok(Some(text))
            },
            layer.is_some(),
        )
    }

    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        Self::transact(Path::new(self.settings_path(scope)), update, false)
    }

    fn with_write_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        Self::transact(Path::new(self.settings_path(scope)), update, true)
    }
}
