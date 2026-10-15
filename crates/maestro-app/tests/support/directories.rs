//! Controlled current-directory observations.
use maestro_app::{ResourceEntry, ResourceFileType, ResourceOperations};
use std::{
    cell::Cell,
    io,
    path::{Path, PathBuf},
};
/// A supplied cwd with a count of required observations.
pub struct Directories {
    /// The supplied directory observation, including failure.
    pub cwd: io::Result<String>,
    /// Number of current-directory requests.
    pub reads: Cell<usize>,
}
impl ResourceOperations for Directories {
    fn exists(&self, _: &Path) -> bool {
        panic!("unexpected exists")
    }
    fn read_dir(&self, _: &Path) -> io::Result<Vec<ResourceEntry>> {
        panic!("unexpected read_dir")
    }
    fn read_file(&self, _: &Path) -> io::Result<String> {
        panic!("unexpected read_file")
    }
    fn metadata(&self, _: &Path) -> io::Result<ResourceFileType> {
        panic!("unexpected metadata")
    }
    fn symlink_metadata(&self, _: &Path) -> io::Result<ResourceFileType> {
        panic!("unexpected symlink_metadata")
    }
    fn canonicalize(&self, _: &Path) -> io::Result<PathBuf> {
        panic!("unexpected canonicalize")
    }
    fn current_directory(&self) -> io::Result<String> {
        self.reads.set(self.reads.get() + 1);
        self.cwd
            .as_ref()
            .cloned()
            .map_err(|_| io::Error::from_raw_os_error(2))
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        Vec::new()
    }
}
