//! Incremental raw UTF-8 decoding with caller-owned framing and BOM policy.

/// Decodes complete scalars while retaining an incomplete trailing scalar.
#[derive(Debug, Default)]
pub struct Utf8Stream {
    /// Bytes of a scalar whose end has not arrived.
    pending: Vec<u8>,
}
impl Utf8Stream {
    /// Returns completed text, replacing maximal invalid prefixes and retaining an incomplete suffix.
    #[must_use]
    pub fn push(&mut self, bytes: &[u8]) -> String {
        let mut pending = std::mem::take(&mut self.pending);
        let mut input = if pending.is_empty() {
            bytes
        } else {
            pending.extend_from_slice(bytes);
            &pending
        };
        let mut text = String::new();
        while !input.is_empty() {
            let error = match std::str::from_utf8(input) {
                Ok(valid) => {
                    text.push_str(valid);
                    break;
                }
                Err(error) => error,
            };
            let (valid, rest) = input.split_at(error.valid_up_to());
            text.push_str(&String::from_utf8_lossy(valid));
            let Some(length) = error.error_len() else {
                self.pending.extend_from_slice(rest);
                break;
            };
            text.push(char::REPLACEMENT_CHARACTER);
            input = &rest[length..];
        }
        text
    }
    /// Replaces a remaining incomplete scalar once and clears the carry.
    #[must_use]
    pub fn finish(&mut self) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned()
    }
}
