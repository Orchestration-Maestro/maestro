//! Incremental UTF-8 decoding that holds an incomplete character until its bytes arrive.

use std::str;

/// Decodes UTF-8 delivered in arbitrary chunks.
#[derive(Debug, Default)]
pub(super) struct Utf8Stream {
    /// The bytes of a character whose end has not arrived.
    pending: Vec<u8>,
}

impl Utf8Stream {
    /// Appends `bytes` and returns the text they complete. An invalid byte sequence becomes one
    /// U+FFFD per maximal invalid prefix; an incomplete trailing character stays pending.
    pub(super) fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut text = String::new();
        while let Err(error) = str::from_utf8(&self.pending) {
            let valid = error.valid_up_to();
            text.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
            let Some(invalid) = error.error_len() else {
                self.pending.drain(..valid);
                return text;
            };
            text.push(char::REPLACEMENT_CHARACTER);
            self.pending.drain(..valid + invalid);
        }
        text.push_str(&String::from_utf8_lossy(&self.pending));
        self.pending.clear();
        text
    }

    /// Returns the replacement for a character that never completed and forgets it.
    pub(super) fn finish(&mut self) -> String {
        let text = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        text
    }
}
