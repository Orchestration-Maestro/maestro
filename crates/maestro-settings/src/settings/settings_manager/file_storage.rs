use super::{Error, Operation, SettingsScope, SettingsStorage, lease::Lease, value};
use std::path::{Path, PathBuf};

/// Native raw-text preferences at explicitly supplied configuration locations.
/// Construction and missing-file reads create no directories.
pub struct FileSettingsStorage {
    paths: [PathBuf; 2],
}
impl FileSettingsStorage {
    /// Selects supplied locations with lexical host join semantics.
    pub fn new(cwd: &Path, agent_dir: &Path, configuration_dir_name: &str) -> Self {
        Self {
            paths: [
                value::join(&[agent_dir, Path::new("settings.json")]),
                value::join(&[
                    cwd,
                    Path::new(configuration_dir_name),
                    Path::new("settings.json"),
                ]),
            ],
        }
    }
}
impl SettingsStorage for FileSettingsStorage {
    fn with_lock<'a>(
        &self,
        scope: SettingsScope,
        operation: &'a mut Operation<'a>,
    ) -> Result<(), Error> {
        transaction(&SystemIo, &self.paths[scope.index()], operation)
    }
}
trait FileIo {
    fn read(&self, path: &Path) -> Result<String, Error>;
    fn create_directory(&self, path: &Path) -> Result<(), Error>;
    fn write(&self, path: &Path, text: &str) -> Result<(), Error>;
}
struct SystemIo;
impl FileIo for SystemIo {
    fn read(&self, path: &Path) -> Result<String, Error> {
        Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
    }
    fn create_directory(&self, path: &Path) -> Result<(), Error> {
        Ok(std::fs::create_dir_all(path)?)
    }
    fn write(&self, path: &Path, text: &str) -> Result<(), Error> {
        Ok(std::fs::write(path, text)?)
    }
}
fn transaction<'a>(
    io: &dyn FileIo,
    path: &Path,
    operation: &'a mut Operation<'a>,
) -> Result<(), Error> {
    let _section = super::lease::Section::enter();
    let exists = path.exists();
    let mut lease = if exists {
        Some(Lease::acquire(path)?)
    } else {
        None
    };
    let result = (|| {
        let current = if exists { Some(io.read(path)?) } else { None };
        if let Some(next) = operation(current.as_deref())? {
            if !path.parent().unwrap().exists() {
                io.create_directory(path.parent().unwrap())?;
            }
            if lease.is_none() {
                lease = Some(Lease::acquire(path)?);
            }
            io.write(path, &next)?;
        }
        Ok(())
    })();
    if let Some(lease) = lease.as_mut() {
        lease.release()?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_in_place_write_keeps_partial_bytes_and_releases() {
        struct Partial;
        impl FileIo for Partial {
            fn read(&self, path: &Path) -> Result<String, Error> {
                SystemIo.read(path)
            }
            fn create_directory(&self, path: &Path) -> Result<(), Error> {
                SystemIo.create_directory(path)
            }
            fn write(&self, path: &Path, text: &str) -> Result<(), Error> {
                std::fs::write(path, &text[..2])?;
                Err(std::io::Error::other("sentinel partial write").into())
            }
        }
        let root =
            std::env::temp_dir().join(format!("maestro-settings-write-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        let error =
            transaction(&Partial, &path, &mut |_| Ok(Some("{\"new\":true}".into()))).unwrap_err();
        assert_eq!(error.to_string(), "sentinel partial write");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"");
        assert!(!root.join("settings.json.lock").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
