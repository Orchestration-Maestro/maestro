//! Synchronous line framing for caller-owned byte streams.

use serde_json::Value;
use std::{fmt, io};

/// Serialize one JSON value with one terminating LF.
///
/// # Errors
/// Returns a failure from the shared compact writer.
pub fn serialize_json_line(value: &Value) -> Result<String, serde_json::Error> {
    serialize_line(value, false)
}

/// Serialize one JSON value with exact integers and one terminating LF.
///
/// Keys and floating-point values follow the compact canonical writer.
///
/// # Errors
/// Returns a failure from the shared compact writer.
pub fn serialize_native_json_line(value: &Value) -> Result<String, serde_json::Error> {
    serialize_line(value, true)
}

/// Frame the selected numeric representation with one terminating LF.
fn serialize_line(value: &Value, preserve_integers: bool) -> Result<String, serde_json::Error> {
    let mut line = if preserve_integers {
        crate::json_text::compact_native_json(value)?
    } else {
        crate::compact_json(value)?
    };
    line.push('\n');
    Ok(line)
}

/// The failure of a strictly framed line whose bytes before its LF exceed the limit.
#[derive(Debug)]
pub struct LineTooLarge;

impl fmt::Display for LineTooLarge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("line is too large")
    }
}

impl std::error::Error for LineTooLarge {}

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
        let lines = self.walk(bytes, usize::MAX, |line| Ok(lossy(&line)));
        lines.into_iter().flatten().collect()
    }

    /// Consume bytes and return completed lines decoded as strict UTF-8, removing LF and
    /// one preceding CR.
    ///
    /// A line fails with invalid data when it is not UTF-8, or with [`LineTooLarge`] once
    /// its buffered bytes, a CR included, exceed `max_bytes`, before its LF arrives. Lines
    /// completed earlier come first; a failure is the last result and drops the rest of
    /// `bytes`.
    #[must_use]
    pub fn push_strict(&mut self, bytes: &[u8], max_bytes: usize) -> Vec<io::Result<String>> {
        self.walk(bytes, max_bytes, |line| {
            String::from_utf8(line)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
    }

    /// Emit a nonempty remaining byte tail, removing one trailing CR.
    #[must_use]
    pub fn finish(&mut self) -> Option<String> {
        (!self.pending.is_empty()).then(|| lossy(&self.take_line()))
    }

    /// Splits `bytes` at each LF and decodes every completed line, failing a line whose
    /// buffered bytes exceed `max_bytes`; stops at the first failure.
    fn walk(
        &mut self,
        bytes: &[u8],
        max_bytes: usize,
        decode: impl Fn(Vec<u8>) -> io::Result<String>,
    ) -> Vec<io::Result<String>> {
        let mut lines = Vec::new();
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            let content = part.strip_suffix(b"\n");
            let added = content.unwrap_or(part);
            if self.pending.len() + added.len() > max_bytes {
                self.pending.clear();
                lines.push(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    LineTooLarge,
                )));
                break;
            }
            self.pending.extend_from_slice(added);
            if content.is_none() {
                continue;
            }
            let line = decode(self.take_line());
            let failed = line.is_err();
            lines.push(line);
            if failed {
                break;
            }
        }
        lines
    }

    /// Takes one line's bytes without its one trailing CR.
    fn take_line(&mut self) -> Vec<u8> {
        let mut bytes = std::mem::take(&mut self.pending);
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        bytes
    }
}

/// Decodes one line, replacing invalid bytes, without carrying decoder state across lines.
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
