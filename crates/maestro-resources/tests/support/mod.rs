//! Disposable native filesystem fixtures.
use maestro_test_directory::Scratch;
use std::path::{Path, PathBuf};

/// Disposable native resource tree.
pub struct Directory(pub Scratch);
impl Directory {
    /// Create a unique tree owned by this test.
    ///
    /// # Panics
    /// Panics when the disposable directory cannot be created.
    pub fn new() -> Self {
        Self(Scratch::new().unwrap())
    }
    /// Write a supplied relative fixture.
    ///
    /// # Panics
    /// Panics when a fixture has no parent or its directory/file cannot be written.
    #[must_use]
    pub fn file(&self, path: &str, content: &str) -> PathBuf {
        let path = self.0.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}
impl Default for Directory {
    fn default() -> Self {
        Self::new()
    }
}

/// Permission bits put back on drop, so the tree can be removed.
#[cfg(unix)]
pub struct Restore(PathBuf, u32);
#[cfg(unix)]
impl Drop for Restore {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
    }
}

/// Take every permission away from `path`. Returns the guard that puts them back and the
/// error reading `path` now gives, or `None` when the run still reads it (a privileged run).
///
/// # Panics
/// Panics when the permissions cannot be read or changed.
#[cfg(unix)]
#[must_use]
pub fn deny(path: &Path) -> Option<(Restore, String)> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).unwrap().permissions().mode();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let guard = Restore(path.to_path_buf(), mode);
    let error = if path.is_dir() {
        std::fs::read_dir(path).err()
    } else {
        std::fs::read(path).err()
    };
    error.map(|error| (guard, error.to_string()))
}

/// Create a link at `link` whose target lies in a directory without search permission: a
/// listing shows the link and following it fails. `None` when a privileged run follows it.
///
/// # Panics
/// Panics when the fixture cannot be created.
#[cfg(unix)]
#[must_use]
pub fn denied_link(dir: &Directory, link: &Path) -> Option<Restore> {
    let name = link.file_name().unwrap().to_str().unwrap();
    let target = dir.file(
        &format!("locked-{name}/target"),
        "---\ndescription: Locked\n---",
    );
    std::os::unix::fs::symlink(&target, link).unwrap();
    deny(target.parent().unwrap()).map(|(guard, _)| guard)
}

/// Borrow a UTF-8 authored fixture path.
///
/// # Panics
/// Panics if a fixture path is not UTF-8.
#[must_use]
pub fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// Convert supplied filesystem fixture paths to authored loader inputs.
#[must_use]
pub fn strings(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|path| text(path).to_owned()).collect()
}
