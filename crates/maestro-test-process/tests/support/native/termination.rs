//! Queued TERM notifications retained until their handler owner is released.

use std::io::{self, Read};
use std::os::unix::net::UnixStream;

/// A queued TERM notification and the registration that owns its write end.
pub(super) struct Termination {
    /// Retained read end, receiving a byte even when TERM precedes the wait.
    reader: UnixStream,
    /// Handler removed before the read end closes on normal return.
    registration: signal_hook::SigId,
}

impl Termination {
    /// Register TERM before readiness so an early signal is retained by the native pipe.
    pub(super) fn new() -> io::Result<Self> {
        let (reader, writer) = UnixStream::pair()?;
        let registration =
            signal_hook::low_level::pipe::register(signal_hook::consts::SIGTERM, writer)?;
        Ok(Self {
            reader,
            registration,
        })
    }

    /// Consume the queued signal notification without polling or a deadline.
    pub(super) fn wait(&mut self) -> io::Result<()> {
        self.reader.read_exact(&mut [0])
    }
}

impl Drop for Termination {
    /// Remove the handler before releasing the retained notification reader.
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.registration);
    }
}
