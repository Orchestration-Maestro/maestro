//! Ordered byte buffers retained across partial writes and readiness waits.

use std::{collections::VecDeque, io, num::NonZeroUsize};

/// Retains ordered buffers and the offset of a partially written front buffer.
#[derive(Default)]
pub struct Writes {
    /// Complete buffers, including the partially written front.
    pending: VecDeque<Vec<u8>>,
    /// Native bytes accepted from the front buffer.
    offset: usize,
    /// Accepted bytes not yet written, including the retained front.
    remaining: usize,
}

/// Result of one nonblocking flush attempt.
pub enum WriteStatus {
    /// All retained bytes were accepted.
    Complete,
    /// Readiness or an interrupted call requires another attempt.
    Blocked,
    /// The native writer accepted zero bytes.
    Zero,
    /// A permanent failure discarded pending bytes.
    Failed(io::Error),
}

impl Writes {
    /// Appends only nonempty writes.
    pub fn enqueue(&mut self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            self.remaining += bytes.len();
            self.pending.push_back(bytes);
        }
    }

    /// Removes the unconsumed front bytes without replaying an accepted prefix.
    pub fn take_front(&mut self) -> Option<Vec<u8>> {
        let mut bytes = self.pending.pop_front()?;
        let offset = std::mem::take(&mut self.offset);
        if offset != 0 {
            bytes.drain(..offset);
        }
        self.remaining -= bytes.len();
        Some(bytes)
    }

    /// Reports whether the terminal has retained output.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Accepted bytes not yet written.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.remaining
    }

    /// Retires the front buffer after its last accepted byte.
    fn advance(&mut self, length: usize) {
        self.remaining -= length;
        self.offset += length;
        if self
            .pending
            .front()
            .is_some_and(|bytes| self.offset == bytes.len())
        {
            self.pending.pop_front();
            self.offset = 0;
        }
    }

    /// Advances offsets without copying a remaining buffer prefix.
    pub fn flush(&mut self, write: impl FnMut(&[u8]) -> io::Result<usize>) -> WriteStatus {
        self.flush_bounded(NonZeroUsize::MAX, write)
    }

    /// Advances buffers with at most `limit` bytes per write request.
    /// The callback must return no more bytes than it was given.
    pub fn flush_bounded(
        &mut self,
        limit: NonZeroUsize,
        mut write: impl FnMut(&[u8]) -> io::Result<usize>,
    ) -> WriteStatus {
        while let Some(bytes) = self.pending.front() {
            let end = self.offset.saturating_add(limit.get()).min(bytes.len());
            match write(&bytes[self.offset..end]) {
                Ok(0) => return WriteStatus::Zero,
                Ok(length) => {
                    self.advance(length);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    return WriteStatus::Blocked;
                }
                Err(error) => {
                    self.pending.clear();
                    self.offset = 0;
                    self.remaining = 0;
                    return WriteStatus::Failed(error);
                }
            }
        }
        WriteStatus::Complete
    }
}

#[cfg(test)]
mod tests;
