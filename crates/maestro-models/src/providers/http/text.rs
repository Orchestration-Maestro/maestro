//! Lenient UTF-8 decoding of response bodies.

use std::borrow::Cow;

/// Decodes body bytes as text the way a default text decoder does.
///
/// One leading byte-order mark is dropped and every maximal invalid sequence becomes
/// U+FFFD. A sequence cut by the end of a chunk waits for the next one.
#[derive(Default)]
pub(crate) struct TextDecoder {
    /// Bytes of a sequence that the previous chunk ended in the middle of.
    pending: Vec<u8>,
    /// Whether text has been produced, so a byte-order mark can no longer open the body.
    started: bool,
}

impl TextDecoder {
    /// Decode one chunk, completing the sequence the previous chunk left open.
    pub(crate) fn decode(&mut self, chunk: &[u8]) -> String {
        let bytes = if self.pending.is_empty() {
            Cow::Borrowed(chunk)
        } else {
            Cow::Owned([self.pending.as_slice(), chunk].concat())
        };
        self.pending.clear();
        let mut text = String::with_capacity(bytes.len());
        let mut pieces = bytes.utf8_chunks().peekable();
        while let Some(piece) = pieces.next() {
            text.push_str(piece.valid());
            let invalid = piece.invalid();
            if invalid.is_empty() {
                continue;
            }
            if pieces.peek().is_none() && is_cut_sequence(invalid) {
                self.pending.extend_from_slice(invalid);
            } else {
                text.push(char::REPLACEMENT_CHARACTER);
            }
        }
        if !self.started && !text.is_empty() {
            self.started = true;
            if text.starts_with('\u{FEFF}') {
                text.remove(0);
            }
        }
        text
    }

    /// End the body: a sequence still cut becomes one replacement character.
    pub(crate) fn finish(self) -> Option<char> {
        (!self.pending.is_empty()).then_some(char::REPLACEMENT_CHARACTER)
    }
}

/// Report whether `bytes` are the start of a valid character that the input ended inside.
fn is_cut_sequence(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_err_and(|error| error.error_len().is_none())
}
