//! Disposable native filesystem fixtures and controlled observations.
use maestro_resources::{
    NativeResourceOperations, ResourceEntry, ResourceFileType, ResourceOperations,
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Disposable native resource tree.
pub struct Directory(pub PathBuf);
impl Directory {
    /// Create a unique tree owned by this test.
    ///
    /// # Panics
    /// Panics when the disposable directory cannot be created.
    pub fn new() -> Self {
        /// Unique directory suffix within the test process.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-resources-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    /// Write a supplied relative fixture.
    ///
    /// # Panics
    /// Panics when a fixture has no parent or its directory/file cannot be written.
    #[must_use]
    pub fn file(&self, path: &str, content: &str) -> PathBuf {
        let path = self.0.join(path);
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
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Canonicalization failure with otherwise native operations.
pub struct Unresolvable;
impl ResourceOperations for Unresolvable {
    fn exists(&self, path: &Path) -> bool {
        NativeResourceOperations.exists(path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        NativeResourceOperations.read_dir(path)
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        NativeResourceOperations.read_file(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        NativeResourceOperations.metadata(path)
    }
    fn canonicalize(&self, _: &Path) -> io::Result<PathBuf> {
        Err(io::Error::other("controlled canonical failure"))
    }
}

/// Controlled directory order and failures layered over disposable native files.
#[derive(Default)]
pub struct Controlled {
    /// Paths whose specified operation returns an injected error.
    pub failures: Vec<(&'static str, PathBuf)>,
    /// Explicit order for selected native directories.
    pub order: Vec<(PathBuf, Vec<&'static str>)>,
}
impl Controlled {
    /// Inject the selected failure without executing native I/O.
    fn check(&self, operation: &str, path: &Path) -> io::Result<()> {
        if self
            .failures
            .iter()
            .any(|(op, p)| *op == operation && p == path)
        {
            Err(io::Error::other(format!("injected {operation}")))
        } else {
            Ok(())
        }
    }
}
impl ResourceOperations for Controlled {
    fn exists(&self, path: &Path) -> bool {
        NativeResourceOperations.exists(path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        self.check("read_dir", path)?;
        let mut entries = NativeResourceOperations.read_dir(path)?;
        if let Some((_, names)) = self.order.iter().find(|(p, _)| p == path) {
            entries.sort_by_key(|e| {
                names
                    .iter()
                    .position(|name| e.name == *name)
                    .unwrap_or(usize::MAX)
            });
        }
        Ok(entries)
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        self.check("read_file", path)?;
        NativeResourceOperations.read_file(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        self.check("metadata", path)?;
        NativeResourceOperations.metadata(path)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.check("canonicalize", path)?;
        NativeResourceOperations.canonicalize(path)
    }
}

/// Resolve relative filesystem requests beneath a supplied fixture directory.
pub struct Rooted(pub PathBuf);
impl ResourceOperations for Rooted {
    fn exists(&self, path: &Path) -> bool {
        NativeResourceOperations.exists(&self.0.join(path))
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        NativeResourceOperations.read_dir(&self.0.join(path))
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        NativeResourceOperations.read_file(&self.0.join(path))
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        NativeResourceOperations.metadata(&self.0.join(path))
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        NativeResourceOperations.canonicalize(&self.0.join(path))
    }
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
