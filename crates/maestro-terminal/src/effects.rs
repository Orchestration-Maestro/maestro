//! Captured terminal effects, owned until the caller drains them.

use crate::Terminal;

/// Effects accumulated in input order within each queue.
///
/// After each write, drain PTY replies, bells, directory changes and clipboard
/// writes. Drains are independent: reading one never clears another.
#[derive(Default)]
pub struct Effects {
    /// Concatenated reply bytes waiting for the PTY.
    pub(crate) pty_reply: Vec<u8>,
    /// Bell events, saturated at the largest representable count.
    pub(crate) bell_count: u16,
    /// A copy of the stored directory for every directory notification.
    pub(crate) pwd_changes: Vec<Vec<u8>>,
    /// Accepted standard-clipboard payloads.
    pub(crate) clipboard_writes: Vec<Vec<u8>>,
}

impl Terminal {
    /// Take all queued PTY reply bytes, leaving an empty queue.
    pub fn take_pty_reply(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.effects.pty_reply)
    }

    /// Take the saturating bell count, resetting it to zero.
    pub fn take_bell_count(&mut self) -> u16 {
        std::mem::take(&mut self.effects.bell_count)
    }

    /// Take owned directory notifications in input order.
    pub fn take_pwd_changes(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.effects.pwd_changes)
    }

    /// Take accepted clipboard writes in input order.
    pub fn take_clipboard_writes(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.effects.clipboard_writes)
    }
}
