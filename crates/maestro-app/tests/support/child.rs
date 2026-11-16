//! Disposable filesystem and child-process probes.
#[cfg(unix)]
/// A directory removed when dropped, on a best-effort basis.
pub struct Scratch(pub maestro_test_directory::Scratch);

#[cfg(unix)]
impl Scratch {
    pub fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().expect("scratch directory is new"))
    }

    pub fn join(&self, name: &str) -> String {
        self.0.path().join(name).to_string_lossy().into_owned()
    }
}

#[path = "probe.rs"]
mod probe;
#[cfg(unix)]
pub use probe::probe;
