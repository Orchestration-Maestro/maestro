//! Shared native command directories and failing output sinks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
pub(crate) use maestro_test_directory::Scratch;
use std::io;
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
