//! Splits a byte stream into lines at `\r\n`, `\r` and `\n`, whatever the chunk boundaries.

use super::text::decode_utf8;

/// Collects bytes and returns each completed line as text.
/// A carriage return completes its line immediately; one following line feed is skipped.
#[derive(Default)]
pub(crate) struct LineDecoder {
    /// Bytes not yet returned as a line.
    buffer: Vec<u8>,
    /// Whether the last consumed byte was a carriage return.
    skip_line_feed: bool,
}

impl LineDecoder {
    /// Add `chunk` and return the lines it completes, without their endings.
    pub(crate) fn decode(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        for &byte in chunk {
            if std::mem::take(&mut self.skip_line_feed) && byte == b'\n' {
                continue;
            }
            if matches!(byte, b'\r' | b'\n') {
                lines.push(decode_utf8(&self.buffer).into_owned());
                self.buffer.clear();
                self.skip_line_feed = byte == b'\r';
            } else {
                self.buffer.push(byte);
            }
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
}
