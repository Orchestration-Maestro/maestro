//! Disposable filesystem and child-process probes.
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
#[cfg(unix)]
/// A directory removed when dropped, on a best-effort basis.
pub struct Scratch(pub PathBuf);

#[cfg(unix)]
impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-footer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("scratch directory is new");
        Self(path)
    }

    pub fn join(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

#[cfg(unix)]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[path = "probe.rs"]
mod probe;
#[cfg(unix)]
pub use probe::probe;
