//! Observations a test makes on a scripted FTP peer while its work runs.

use std::sync::atomic::Ordering;

use super::ftp_server::Peer;

impl Peer {
    /// Wait until the server logged `count` lines starting with `line`.
    pub async fn reached(&self, line: &str, count: usize) {
        let mut log = self.observed.commands.subscribe();
        // The peer holds the sender, so the log stays open while it is awaited.
        log.wait_for(|log| log.iter().filter(|seen| seen.starts_with(line)).count() >= count)
            .await
            .ok();
    }

    /// The bytes the server wrote to its connections so far.
    pub fn written(&self) -> usize {
        self.observed.written.load(Ordering::SeqCst)
    }
}
