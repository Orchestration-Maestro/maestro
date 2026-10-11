//! Strict recorded recipes and complete public-result comparisons.
#![cfg(test)]
use super::support;
use maestro_packages::{DefaultPackageManager, PackageManagerOptions};
use maestro_resources::{
    NativeResourceOperations,
    skills::{ResourceEntry, ResourceFileType, ResourceOperations},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    rc::Rc,
};
/// Native filesystem with selected read/stat failures.
#[derive(Default)]
pub struct Filesystem {
    /// Reject ambient resource-directory reads.
    pub fail_current: bool,
    /// Native error code returned by an ambient directory read.
    pub current_error_code: Option<i32>,
    /// Number of resource-directory reads.
    pub current_reads: Rc<std::cell::Cell<usize>>,
    /// Replacement extension entry sequence.
    pub order: Vec<String>,
    /// Completed synchronous effect before a directory observation.
    pub on_read: Option<ReadEffect>,
    /// Supplied canonicalization witness.
    pub canonical: Option<Canonical>,
    /// Failure suffixes keyed by operation.
    pub fail: BTreeMap<String, Vec<String>>,
}
/// Supplied synchronous directory-observation effect.
pub type ReadEffect = Rc<dyn Fn(&Path) -> io::Result<()>>;
/// Supplied canonicalization operation.
pub type Canonical = Rc<dyn Fn(&Path) -> io::Result<PathBuf>>;
impl Filesystem {
    /// Whether the recorded operation fails here.
    fn check(&self, operation: &str, path: &Path) -> io::Result<()> {
        if self
            .fail
            .get(operation)
            .is_some_and(|ends| ends.iter().any(|s| path.to_string_lossy().ends_with(s)))
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
}
impl ResourceOperations for Filesystem {
    fn exists(&self, path: &Path) -> bool {
        NativeResourceOperations.exists(path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        self.check("readdirSync", path)?;
        if let Some(effect) = &self.on_read {
            effect(path)?;
        }
        if path.ends_with("extensions") && !self.order.is_empty() {
            return Ok(self
                .order
                .iter()
                .map(|name| ResourceEntry {
                    name: name.into(),
                    file_type: ResourceFileType::File,
                })
                .collect());
        }
        NativeResourceOperations.read_dir(path)
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        self.check("readFileSync", path)?;
        NativeResourceOperations.read_file(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        self.check("statSync", path)?;
        NativeResourceOperations.metadata(path)
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        self.check("lstatSync", path)?;
        NativeResourceOperations.symlink_metadata(path)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        if let Some(canonical) = &self.canonical {
            return canonical(path);
        }
        NativeResourceOperations.canonicalize(path)
    }
    fn current_directory(&self) -> io::Result<String> {
        self.current_reads.set(self.current_reads.get() + 1);
        if let Some(code) = self.current_error_code {
            return Err(io::Error::from_raw_os_error(code));
        }
        if self.fail_current {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        NativeResourceOperations.current_directory()
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        NativeResourceOperations.drive_directories()
    }
}
pub use super::resource_output::output;
/// Constructs a native-file manager with controlled package effects.
pub fn manager(
    base: &str,
    resources: Filesystem,
) -> (
    DefaultPackageManager<support::Controlled>,
    Rc<support::Effects>,
) {
    let (_, settings, _) = support::manager(&json!({}));
    let effects = Rc::new(support::Effects {
        native_exists: true,
        resources: Some(Rc::new(resources)),
        ..Default::default()
    });
    (
        DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: base.into(),
                agent_dir: base.into(),
                settings_manager: settings,
            },
            support::Controlled(Rc::clone(&effects)),
        ),
        effects,
    )
}

/// Drives boundary-test futures through the native Rust runtime.
pub fn run(operation: impl std::future::Future<Output = io::Result<()>>) -> io::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(operation)
}
