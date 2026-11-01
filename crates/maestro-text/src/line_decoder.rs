//! Byte-line framing independent of text decoding.

/// Buffers bytes until CR, LF or CRLF ends a line.
#[derive(Default)]
pub struct ByteLineDecoder {
    /// Bytes not yet returned as a line.
    buffer: Vec<u8>,
    /// Whether one following LF belongs to the preceding CR.
    skip_line_feed: bool,
}

impl ByteLineDecoder {
    /// Returns completed lines without their separators, retaining unfinished bytes.
    pub fn decode(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        let mut lines = Vec::new();
        for &byte in chunk {
            if std::mem::take(&mut self.skip_line_feed) && byte == b'\n' {
                continue;
            }
            if matches!(byte, b'\r' | b'\n') {
                lines.push(std::mem::take(&mut self.buffer));
                self.skip_line_feed = byte == b'\r';
            } else {
                self.buffer.push(byte);
            }
        }
        lines
    }

    /// Returns a final unterminated line, if any.
    pub fn flush(&mut self) -> Vec<Vec<u8>> {
        if self.buffer.is_empty() {
            Vec::new()
        } else {
            vec![std::mem::take(&mut self.buffer)]
        }
    }
}
