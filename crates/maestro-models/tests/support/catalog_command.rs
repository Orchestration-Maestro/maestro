//! Shared native command directories and failing output sinks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use std::{fs, io, path::PathBuf};
/// Own native command output files.
pub(crate) struct Scratch(pub(crate) PathBuf);
impl Scratch {
    /// Allocate a unique command directory.
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-command-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Fail a writer at an exact byte boundary with an identifiable native cause.
pub(crate) struct FailingWriter {
    /// Accepted prefix.
    pub(crate) bytes: Vec<u8>,
    /// Remaining capacity.
    pub(crate) remaining: usize,
}
impl io::Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::from_raw_os_error(28));
        }
        let size = bytes.len().min(self.remaining);
        self.bytes.extend_from_slice(&bytes[..size]);
        self.remaining -= size;
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
