//! Synchronous line framing for caller-owned byte streams.

use serde_json::Value;

/// Serialize one JSON value with one terminating LF.
///
/// # Errors
/// Returns a failure from the shared compact writer.
pub fn serialize_json_line(value: &Value) -> Result<String, serde_json::Error> {
    let mut line = crate::compact_json(value)?;
    line.push('\n');
    Ok(line)
}

/// Holds the unfinished bytes of one line.
#[derive(Debug, Default)]
pub struct JsonlLineSplitter {
    /// Bytes retained until LF or end of input.
    pending: Vec<u8>,
}

impl JsonlLineSplitter {
    /// Consume bytes and return completed lines, removing LF and one preceding CR.
    #[must_use]
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            if let Some(content) = part.strip_suffix(b"\n") {
                self.pending.extend_from_slice(content);
                lines.push(self.take_line());
            } else {
                self.pending.extend_from_slice(part);
            }
        }
        lines
    }

    /// Emit a nonempty remaining byte tail, removing one trailing CR.
    #[must_use]
    pub fn finish(&mut self) -> Option<String> {
        (!self.pending.is_empty()).then(|| self.take_line())
    }

    /// Decode one complete line without carrying decoder state across lines.
    fn take_line(&mut self) -> String {
        let bytes = std::mem::take(&mut self.pending);
        let content = bytes.strip_suffix(b"\r").unwrap_or(&bytes);
        String::from_utf8_lossy(content).into_owned()
    }
}
