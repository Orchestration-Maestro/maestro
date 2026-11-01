//! Splits a byte stream into lines at `\r\n`, `\r` and `\n`, whatever the chunk boundaries.

use maestro_text::decode_utf8;

/// Collects bytes and returns each completed line as text.
/// A carriage return completes its line immediately; one following line feed is skipped.
#[derive(Default)]
pub(crate) struct LineDecoder {
    /// Shared raw-byte framing; UTF-8 and BOM handling stay with this adapter.
    bytes: maestro_text::line_decoder::ByteLineDecoder,
}

impl LineDecoder {
    /// Add `chunk` and return the lines it completes, without their endings.
    pub(crate) fn decode(&mut self, chunk: &[u8]) -> Vec<String> {
        self.bytes
            .decode(chunk)
            .into_iter()
            .map(|line| decode_utf8(&line).into_owned())
            .collect()
    }

    /// End the input: the bytes still held form a last line.
    pub(crate) fn flush(&mut self) -> Vec<String> {
        self.bytes
            .flush()
            .into_iter()
            .map(|line| decode_utf8(&line).into_owned())
            .collect()
    }
}
