//! Native streaming-output accounting and overflow artifacts.

use super::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationBy, TruncationOptions, TruncationResult,
    truncate_tail,
};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};

/// Independent stream budgets and artifact filename prefix.
#[derive(Default)]
pub struct OutputAccumulatorOptions {
    /// Maximum newline entries, or the selection default.
    pub max_lines: Option<usize>,
    /// Maximum decoded bytes, or the selection default.
    pub max_bytes: Option<usize>,
    /// Authored filename prefix; absent selects `maestro-output`.
    pub temp_file_prefix: Option<String>,
}
/// Selected decoded output and optional artifact identity.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputSnapshot {
    /// Selected text, also present in the accounting record.
    pub content: String,
    /// Selection accounting.
    pub truncation: TruncationResult,
    /// Artifact path; successful explicit close establishes completed writes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_output_path: Option<String>,
}
/// One native output stream.
pub struct OutputAccumulator {
    /// Selected budgets.
    options: TruncationOptions,
    /// Whether decoder finalization has run.
    finished: bool,
    /// Whether any decoded scalar has been observed.
    started: bool,
    /// Incomplete scalar suffix, at most three bytes.
    pending: Vec<u8>,
    /// Artifact filename prefix.
    prefix: String,
    /// Unspilled original bytes.
    raw: Vec<u8>,
    /// Total original byte count.
    raw_bytes: usize,
    /// Rolling trim target; trigger is twice this value.
    rolling_bytes: usize,
    /// Whether retained text starts at a complete line boundary.
    line_boundary: bool,
    /// Decoded display text.
    tail: String,
    /// Complete decoded byte count.
    decoded_bytes: usize,
    /// Complete newline-entry count.
    lines: usize,
    /// Current decoded line's byte count.
    last_line_bytes: usize,
    /// Artifact path once opened.
    path: Option<String>,
    /// Owned buffered artifact writer.
    writer: Option<BufWriter<File>>,
}
impl OutputAccumulator {
    /// Construct without filesystem effects.
    #[must_use]
    pub fn new(options: OutputAccumulatorOptions) -> Self {
        Self {
            options: TruncationOptions {
                max_lines: Some(options.max_lines.unwrap_or(DEFAULT_MAX_LINES)),
                max_bytes: Some(options.max_bytes.unwrap_or(DEFAULT_MAX_BYTES)),
            },
            rolling_bytes: options
                .max_bytes
                .unwrap_or(DEFAULT_MAX_BYTES)
                .saturating_mul(2)
                .max(1),
            line_boundary: true,
            prefix: options
                .temp_file_prefix
                .unwrap_or_else(|| "maestro-output".into()),
            finished: false,
            started: false,
            pending: Vec::new(),
            raw: Vec::new(),
            raw_bytes: 0,
            tail: String::new(),
            decoded_bytes: 0,
            lines: 1,
            last_line_bytes: 0,
            path: None,
            writer: None,
        }
    }
    /// Append raw output bytes.
    ///
    /// # Errors
    /// Returns an error after finish or artifact closure, or on artifact creation or writing.
    /// Callers must stop using the accumulator after an I/O error.
    pub fn append(&mut self, data: &[u8]) -> io::Result<()> {
        if self.finished {
            return Err(io::Error::other(
                "Cannot append to a finished output accumulator",
            ));
        }
        if self.path.is_some() && self.writer.is_none() {
            return Err(io::Error::other(
                "Cannot append to a closed output artifact",
            ));
        }
        self.raw_bytes += data.len();
        self.decode(data);
        if self.writer.is_some() || self.should_spill() {
            self.ensure_file()?;
            if let Some(writer) = &mut self.writer {
                writer.write_all(data)?;
            }
        } else {
            self.raw.extend_from_slice(data);
        }
        Ok(())
    }
    /// Flush pending UTF-8 once without closing the artifact.
    ///
    /// # Errors
    /// Returns errors from artifact creation or writing.
    pub fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        if !self.pending.is_empty() {
            self.pending.clear();
            self.append_text("\u{fffd}");
        }
        if self.should_spill() {
            self.ensure_file()?;
        }
        Ok(())
    }
    /// Observe selected decoded output without finishing decoding.
    ///
    /// # Errors
    /// Returns an artifact creation or writing error when persistence is
    /// requested for truncated output.
    pub fn snapshot(&mut self, persist_if_truncated: bool) -> io::Result<OutputSnapshot> {
        let mut truncation = truncate_tail(self.snapshot_text(), self.options);
        truncation.truncated =
            self.lines > truncation.max_lines || self.decoded_bytes > truncation.max_bytes;
        if truncation.truncated && truncation.truncated_by.is_none() {
            truncation.truncated_by = Some(if self.decoded_bytes > truncation.max_bytes {
                TruncationBy::Bytes
            } else {
                TruncationBy::Lines
            });
        }
        truncation.total_lines = self.lines;
        truncation.total_bytes = self.decoded_bytes;
        if persist_if_truncated && truncation.truncated {
            self.ensure_file()?;
        }
        Ok(OutputSnapshot {
            content: truncation.content.clone(),
            truncation,
            full_output_path: self.path.clone(),
        })
    }
    /// Close the artifact writer without finishing decoding or discarding observations.
    /// Artifacts remain on disk; this does not request filesystem durability.
    ///
    /// # Errors
    /// Returns the writer's flush error; the artifact is not known complete.
    pub fn close_temp_file(&mut self) -> io::Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
        }
        Ok(())
    }
    /// Return the current decoded line's byte count.
    #[must_use]
    pub fn get_last_line_bytes(&self) -> usize {
        self.last_line_bytes
    }

    /// Decode complete scalars, retaining only an incomplete suffix.
    fn decode(&mut self, data: &[u8]) {
        let mut pending = std::mem::take(&mut self.pending);
        let mut input = if pending.is_empty() {
            data
        } else {
            pending.extend_from_slice(data);
            &pending
        };
        let mut decoded = String::new();
        while !input.is_empty() {
            let error = match std::str::from_utf8(input) {
                Ok(text) => {
                    decoded.push_str(text);
                    break;
                }
                Err(error) => error,
            };
            let (valid, rest) = input.split_at(error.valid_up_to());
            decoded.push_str(&String::from_utf8_lossy(valid));
            let Some(length) = error.error_len() else {
                self.pending.extend_from_slice(rest);
                break;
            };
            decoded.push('\u{fffd}');
            input = &rest[length..];
        }
        self.append_text(&decoded);
    }

    /// Account for newly decoded text once.
    fn append_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let text = if self.started {
            text
        } else {
            self.started = true;
            text.strip_prefix('\u{feff}').unwrap_or(text)
        };
        self.decoded_bytes += text.len();
        self.tail.push_str(text);
        if self.tail.len() > self.rolling_bytes.saturating_mul(2) {
            self.trim_tail();
        }
        let newlines = text.bytes().filter(|&byte| byte == b'\n').count();
        self.lines += newlines;
        self.last_line_bytes = text
            .rfind('\n')
            .map_or(self.last_line_bytes + text.len(), |i| text.len() - i - 1);
    }
    /// Evict old text only beyond the hysteresis threshold.
    fn trim_tail(&mut self) {
        let mut start = self.tail.len() - self.rolling_bytes;
        while !self.tail.is_char_boundary(start) {
            start += 1;
        }
        if start > 0 {
            self.line_boundary = self.tail.as_bytes()[start - 1] == b'\n';
        }
        self.tail = self.tail[start..].to_owned();
    }
    /// Exclude a leading fragment only when its terminating newline is retained.
    fn snapshot_text(&self) -> &str {
        if self.line_boundary {
            &self.tail
        } else {
            self.tail
                .find('\n')
                .map_or(&self.tail, |i| &self.tail[i + 1..])
        }
    }
    /// Determine whether original or decoded output exceeds a budget.
    fn should_spill(&self) -> bool {
        self.raw_bytes > self.options.max_bytes.unwrap_or(DEFAULT_MAX_BYTES)
            || self.decoded_bytes > self.options.max_bytes.unwrap_or(DEFAULT_MAX_BYTES)
            || self.lines > self.options.max_lines.unwrap_or(DEFAULT_MAX_LINES)
    }
    /// Open an exclusive artifact and replay the retained original prefix.
    fn ensure_file(&mut self) -> io::Result<()> {
        if self.path.is_some() || self.writer.is_some() {
            return Ok(());
        }
        let mut random = [0_u8; 8];
        getrandom::fill(&mut random).map_err(io::Error::other)?;
        let id: String = random
            .iter()
            .flat_map(|byte| [byte >> 4, byte & 0xf])
            .map(|digit| char::from(b"0123456789abcdef"[usize::from(digit)]))
            .collect();
        let root = std::env::temp_dir();
        let root = root.to_string_lossy();
        let path = maestro_path::join(&[&root, &format!("{}-{id}.log", self.prefix)]);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&self.raw)?;
        self.raw = Vec::new();
        self.path = Some(path);
        self.writer = Some(writer);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
