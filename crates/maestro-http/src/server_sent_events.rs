//! Reads a response body as server-sent events.

use maestro_text::line_framing::ByteLineDecoder;
use maestro_text::{decode_utf8, lossy_string};

#[cfg(test)]
mod tests;

/// One event with an optional name and a payload selected by the framing mode.
#[derive(Debug, PartialEq, Eq)]
pub struct ServerSentEvent {
    /// The last `event` field of the event, if it had one.
    pub event: Option<String>,
    /// Joined data fields in field mode, or the payload/non-data text in segment mode.
    pub data: String,
    /// Whether `data` is event data rather than a complete non-data segment.
    pub is_data: bool,
    /// Every nonblank line since the previous event, comments and unknown fields included;
    /// empty unless [`SseOptions::raw_lines`] asks for it.
    pub raw: Vec<String>,
}

/// The ways a reader differs from the default framing.
#[derive(Clone, Copy, Default)]
pub struct SseOptions {
    /// Drop a byte-order mark from the start of the stream only, not from every line.
    pub stream_start_mark: bool,
    /// Keep the lines of each event in [`ServerSentEvent::raw`].
    pub raw_lines: bool,
    /// Dispatch the event being collected when the body ends.
    pub flush_pending: bool,
}

/// Turns lines into events: `event` and `data` fields collect until a blank line dispatches
/// them; comments and every other field are ignored.
#[derive(Default)]
struct SseDecoder {
    /// Name given by the latest `event` field.
    event: Option<String>,
    /// Values of the `data` fields so far.
    data: Vec<String>,
    /// Nonblank lines since the last dispatch, when kept.
    raw: Vec<String>,
    /// Whether the nonblank lines are kept.
    keep_raw: bool,
}

impl SseDecoder {
    /// Read one line, returning the event a blank line dispatches.
    fn decode(&mut self, line: &str) -> Option<ServerSentEvent> {
        if line.is_empty() {
            return self.dispatch();
        }
        if self.keep_raw {
            self.raw.push(line.to_owned());
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => self.data.push(value.to_owned()),
            _ => {}
        }
        None
    }

    /// Dispatch the event collected so far. Nothing is dispatched while neither a nonempty
    /// event name nor a data field has been seen.
    fn dispatch(&mut self) -> Option<ServerSentEvent> {
        if self.event.as_deref().is_none_or(str::is_empty) && self.data.is_empty() {
            return None;
        }
        Some(ServerSentEvent {
            event: self.event.take(),
            data: std::mem::take(&mut self.data).join("\n"),
            is_data: true,
            raw: std::mem::take(&mut self.raw),
        })
    }
}

/// Reads server-sent events from body chunks.
///
/// In field mode, completed lines are read as each chunk arrives. CR, LF and CRLF end a
/// line; CR ends its line immediately and one following LF is skipped, even across chunks.
/// UTF-8 bytes stay buffered until their line ends. A blank line dispatches the collected
/// event. Data-segment mode instead preserves each completed segment's payload. Each line
/// loses a leading byte-order mark unless [`SseOptions::stream_start_mark`] limits that to
/// the first line.
#[derive(Default)]
pub struct SseMessages {
    /// Splits body chunks into lines.
    lines: ByteLineDecoder,
    /// How this reader differs from the default framing.
    options: SseOptions,
    /// Whether the first line of the stream has been read.
    started: bool,
    /// Collects the fields of the lines.
    fields: SseDecoder,
    /// Whole-segment framing, when selected instead of fields.
    segments: Option<Segments>,
}

impl SseMessages {
    /// A reader of field framing that differs from the default as `options` say.
    #[must_use]
    pub fn with_options(options: SseOptions) -> Self {
        Self {
            fields: SseDecoder {
                keep_raw: options.raw_lines,
                ..SseDecoder::default()
            },
            options,
            ..Self::default()
        }
    }

    /// Add a body chunk and return the events it completes.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<ServerSentEvent> {
        if let Some(segments) = &mut self.segments {
            return segments.push(chunk);
        }
        let mut events = Vec::new();
        self.read(chunk, &mut events);
        events
    }

    /// In field mode, read a final unterminated line; an event whose blank line never
    /// arrived is delivered only when [`SseOptions::flush_pending`] asks. Data-segment mode
    /// returns no events.
    pub fn finish(&mut self) -> Vec<ServerSentEvent> {
        if self.segments.is_some() {
            return Vec::new();
        }
        let mut events = Vec::new();
        for line in self.lines.flush() {
            let line = self.text(line);
            events.extend(self.fields.decode(&line));
        }
        if self.options.flush_pending {
            events.extend(self.fields.dispatch());
        }
        events
    }

    /// Decode `bytes` into lines and collect the events they complete.
    fn read(&mut self, bytes: &[u8], events: &mut Vec<ServerSentEvent>) {
        for line in self.lines.decode(bytes) {
            let line = self.text(line);
            events.extend(self.fields.decode(&line));
        }
    }

    /// Decode a line as text: invalid sequences become U+FFFD and a byte-order mark is dropped.
    fn text(&mut self, bytes: Vec<u8>) -> String {
        if !self.options.stream_start_mark {
            return decode_utf8(&bytes).into_owned();
        }
        let mut text = lossy_string(bytes);
        if !std::mem::replace(&mut self.started, true) && text.starts_with('\u{feff}') {
            text.remove(0);
        }
        text
    }
}

/// Byte buffer with a monotone delimiter scan for whole data segments.
#[derive(Default)]
struct Segments {
    /// Bytes after the last complete delimiter.
    buffer: Vec<u8>,
    /// First position not yet ruled out as a delimiter start.
    scan: usize,
}

impl Segments {
    /// Advance a monotone delimiter scan with bounded lookbehind across chunks.
    fn push(&mut self, chunk: &[u8]) -> Vec<ServerSentEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut start = 0;
        let mut events = Vec::new();
        while self.scan + 1 < self.buffer.len() {
            let remaining = &self.buffer[self.scan..];
            if remaining.len() < 4 && b"\r\n\r\n".starts_with(remaining) {
                break;
            }
            let length = if remaining.starts_with(b"\n\n") || remaining.starts_with(b"\r\r") {
                2
            } else if remaining.starts_with(b"\r\n\r\n") {
                4
            } else {
                self.scan += 1;
                continue;
            };
            let text = super::decode_utf8(&self.buffer[start..self.scan]);
            let text = text.trim_matches(maestro_text::is_whitespace_scalar);
            let data = text.strip_prefix("data:");
            events.push(ServerSentEvent {
                event: None,
                is_data: data.is_some(),
                raw: Vec::new(),
                data: data
                    .unwrap_or(text)
                    .trim_matches(maestro_text::is_whitespace_scalar)
                    .to_owned(),
            });
            self.scan += length;
            start = self.scan;
        }
        self.buffer.drain(..start);
        self.scan -= start;
        events
    }
}

impl SseMessages {
    /// Preserve LF-LF, CR-CR or CRLF-CRLF segments with their data-field classification.
    #[must_use]
    pub fn data_segments() -> Self {
        Self {
            segments: Some(Segments::default()),
            ..Self::default()
        }
    }

    /// Decode the remaining segment without flushing an incomplete UTF-8 suffix.
    #[must_use]
    pub fn segment_tail(&self) -> String {
        let bytes = self
            .segments
            .as_ref()
            .map_or(&[][..], |segments| segments.buffer.as_slice());
        let mut end = bytes.len();
        let start = end.saturating_sub(3);
        for index in start..end {
            if bytes[index] & 0b1100_0000 != 0b1000_0000
                && std::str::from_utf8(&bytes[index..])
                    .is_err_and(|error| error.error_len().is_none())
            {
                end = index;
                break;
            }
        }
        super::decode_utf8(&bytes[..end]).into_owned()
    }
}
