//! Splits a byte stream into lines at `\r\n`, `\r` and `\n`, whatever the chunk boundaries.

use super::text::decode_utf8;

/// Collects bytes and returns each completed line as text.
///
/// A line that ends at a carriage return is returned only when the next line ending arrives or
/// the input is flushed, which shows whether that carriage return began `\r\n`.
#[derive(Default)]
pub(crate) struct LineDecoder {
    /// Bytes not yet returned as a line.
    buffer: Vec<u8>,
    /// Position just after a carriage return that has not yet ended a line.
    after_carriage: Option<usize>,
}

impl LineDecoder {
    /// Add `chunk` and return the lines it completes, without their endings.
    pub(crate) fn decode(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(at) = self.next_ending() {
            let carriage = self.buffer[at] == b'\r';
            let after = at + 1;
            if carriage && self.after_carriage.is_none() {
                self.after_carriage = Some(after);
                continue;
            }
            let (end, consumed) = match self.after_carriage {
                Some(pending) if after != pending + 1 || carriage => (pending - 1, pending),
                Some(_) => (at - 1, after),
                None => (at, after),
            };
            lines.push(decode_utf8(&self.buffer[..end]).into_owned());
            self.buffer.drain(..consumed);
            self.after_carriage = None;
        }
        lines
    }

    /// End the input: the bytes still held form a last line.
    pub(crate) fn flush(&mut self) -> Vec<String> {
        if self.buffer.is_empty() {
            Vec::new()
        } else {
            self.decode(b"\n")
        }
    }

    /// Position of the next carriage return or line feed after the held-back carriage return.
    fn next_ending(&self) -> Option<usize> {
        let from = self.after_carriage.unwrap_or(0);
        let offset = self.buffer[from..]
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))?;
        Some(from + offset)
    }
}

/// Position just after the first blank line (`\n\n`, `\r\r` or `\r\n\r\n`) in `buffer`.
pub(crate) fn find_double_newline(buffer: &[u8]) -> Option<usize> {
    (0..buffer.len()).find_map(|start| match buffer[start..] {
        [b'\n', b'\n', ..] | [b'\r', b'\r', ..] => Some(start + 2),
        [b'\r', b'\n', b'\r', b'\n', ..] => Some(start + 4),
        _ => None,
    })
}
