//! Byte-line framing independent of text decoding.

/// Buffers bytes until CR, LF or CRLF ends a line.
#[derive(Default)]
pub struct ByteLineDecoder {
    /// Bytes not yet returned as a line.
    buffer: Vec<u8>,
}

impl ByteLineDecoder {
    /// Returns completed lines without their separators, retaining unfinished bytes.
    pub fn decode(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(chunk);
        Vec::new()
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
