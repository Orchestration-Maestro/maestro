//! Server-sent event framing of a message stream, the repair of event data and the point at
//! which the stream is complete.
//!
//! This framing is the message protocol's own: a carriage return ends its line at once and
//! swallows one line feed that follows it, even in the next chunk; one byte-order mark is
//! dropped from the start of the stream; and a stream is complete only at its stop event.

use futures_util::StreamExt;
use serde_json::value::RawValue;

use super::ABORTED_TEXT;
use super::events::{Progress, Reducer};
use super::wire::{self, Event};
use crate::Cancellation;
use crate::arguments::repair_json;
use crate::providers::http::{FetchError, HttpBody, Raced, RequestFailure, race};

/// Text of the failure of a stream that ends before its stop event.
const UNTERMINATED_TEXT: &str = "Anthropic stream ended before message_stop";
/// The event names whose data is a message event.
const MESSAGE_EVENTS: [&str; 6] = [
    "message_start",
    "message_delta",
    "message_stop",
    "content_block_start",
    "content_block_delta",
    "content_block_stop",
];

/// One event of the stream with the lines that formed it.
struct Frame {
    /// The last `event` field of the event, if it had one.
    event: Option<String>,
    /// The `data` fields joined by `\n`.
    data: String,
    /// Every nonblank line since the previous event, comments and unknown fields included.
    raw: Vec<String>,
}

/// Turns bytes into events: lines end at `\r\n`, `\r` or `\n`; `event` and `data` fields
/// collect until a blank line dispatches them.
#[derive(Default)]
struct Decoder {
    /// Bytes of a character cut by the end of a chunk.
    pending: Vec<u8>,
    /// Whether the first text of the stream, which may start with a byte-order mark, is still
    /// to come.
    at_start: bool,
    /// Whether the previous line ended at a carriage return, so that a line feed next is part
    /// of the same ending.
    after_carriage_return: bool,
    /// The line being read.
    line: String,
    /// Name given by the latest `event` field.
    event: Option<String>,
    /// Values of the `data` fields so far.
    data: Vec<String>,
    /// Lines since the last dispatch.
    raw: Vec<String>,
}

impl Decoder {
    /// A decoder at the start of a stream.
    fn new() -> Self {
        Self {
            at_start: true,
            ..Self::default()
        }
    }

    /// Add a chunk and return the events it completes.
    fn push(&mut self, chunk: &[u8]) -> Vec<Frame> {
        let text = self.text(chunk);
        let mut frames = Vec::new();
        for character in text.chars() {
            if std::mem::take(&mut self.after_carriage_return) && character == '\n' {
                continue;
            }
            match character {
                '\r' | '\n' => {
                    self.after_carriage_return = character == '\r';
                    let line = std::mem::take(&mut self.line);
                    frames.extend(self.read(&line));
                }
                other => self.line.push(other),
            }
        }
        frames
    }

    /// End the stream: a character cut at the end becomes U+FFFD, a last line without its
    /// ending is read and the event being collected is dispatched.
    fn finish(&mut self) -> Vec<Frame> {
        if !self.pending.is_empty() {
            self.line.push(char::REPLACEMENT_CHARACTER);
            self.pending.clear();
        }
        let mut frames = Vec::new();
        if !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            frames.extend(self.read(&line));
        }
        frames.extend(self.dispatch());
        frames
    }

    /// Decode a chunk as text: invalid sequences become U+FFFD, a character cut by the end of
    /// the chunk is completed by the next one, and one byte-order mark is dropped from the
    /// start of the stream.
    fn text(&mut self, chunk: &[u8]) -> String {
        self.pending.extend_from_slice(chunk);
        let bytes = std::mem::take(&mut self.pending);
        let mut rest = bytes.as_slice();
        let mut text = String::new();
        while let Err(error) = std::str::from_utf8(rest) {
            let (valid, invalid) = rest.split_at(error.valid_up_to());
            text.push_str(&String::from_utf8_lossy(valid));
            if let Some(length) = error.error_len() {
                text.push(char::REPLACEMENT_CHARACTER);
                rest = &invalid[length..];
            } else {
                self.pending = invalid.to_vec();
                rest = &[];
            }
        }
        text.push_str(&String::from_utf8_lossy(rest));
        if self.at_start && !text.is_empty() {
            self.at_start = false;
            if text.starts_with('\u{feff}') {
                text.remove(0);
            }
        }
        text
    }

    /// Read one line; a blank line dispatches the event collected so far.
    fn read(&mut self, line: &str) -> Option<Frame> {
        if line.is_empty() {
            return self.dispatch();
        }
        self.raw.push(line.to_owned());
        if line.starts_with(':') {
            return None;
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

    /// Dispatch the event collected so far, if it has a nonempty name or any data field.
    fn dispatch(&mut self) -> Option<Frame> {
        if self.event.as_deref().is_none_or(str::is_empty) && self.data.is_empty() {
            return None;
        }
        Some(Frame {
            event: self.event.take(),
            data: std::mem::take(&mut self.data).join("\n"),
            raw: std::mem::take(&mut self.raw),
        })
    }
}

/// Read event data as JSON, retrying with the repaired text only when the first reading fails;
/// the failure is that of the last reading.
fn parse(data: &str) -> Result<Box<RawValue>, serde_json::Error> {
    serde_json::from_str(data).or_else(|original| {
        let repaired = repair_json(data);
        if repaired == data {
            Err(original)
        } else {
            serde_json::from_str(&repaired)
        }
    })
}

/// Read the message event a frame carries: `None` for frames that carry none.
///
/// # Errors
/// Fails on a frame with an `error` event name, whose data is the failure text, on event
/// data that is not JSON or is `null`, and on a known event that lacks a member it requires.
fn event(frame: &Frame) -> Result<Option<Event>, RequestFailure> {
    let name = frame.event.as_deref().unwrap_or_default();
    if name == "error" {
        return Err(RequestFailure::new(frame.data.clone()));
    }
    if !MESSAGE_EVENTS.contains(&name) {
        return Ok(None);
    }
    let unreadable = |cause: serde_json::Error| {
        RequestFailure::new(format!(
            "Could not parse Anthropic SSE event {name}: {cause}; data={}; raw={}",
            frame.data,
            frame.raw.join("\\n")
        ))
    };
    let raw = parse(&frame.data).map_err(unreadable)?;
    let Some(kind) = wire::event_type(&raw).map_err(unreadable)? else {
        return Ok(None);
    };
    wire::decode(&kind, &raw).map_err(|cause| RequestFailure::new(cause.to_string()))
}

/// How a body is read.
#[derive(Clone, Copy)]
pub(super) enum Reading {
    /// Reads race the cancellation signal, as a transport's own reads do.
    Transport,
    /// Reads of a body an injected client supplied end on their own; the signal is checked
    /// before each read.
    Injected,
}

/// Read the response body as server-sent events, reducing each message event until the stop
/// event arrives; the body is dropped there without reading the rest.
///
/// # Errors
/// Fails on a cancelled signal, a transport error, an `error` event, event data that cannot be
/// read, a reduction failure and a body that ends before its stop event.
pub(super) async fn consume(
    mut body: HttpBody,
    signal: Option<&Cancellation>,
    reading: Reading,
    reducer: &mut Reducer<'_>,
) -> Result<(), RequestFailure> {
    let mut decoder = Decoder::new();
    loop {
        if signal.is_some_and(Cancellation::is_aborted) {
            return Err(RequestFailure::new(ABORTED_TEXT));
        }
        let chunk = match reading {
            Reading::Injected => body.next().await,
            Reading::Transport => match race(body.next(), None, signal).await {
                Raced::Done(chunk) => chunk,
                Raced::Cancelled | Raced::TimedOut => {
                    return Err(RequestFailure::new(ABORTED_TEXT));
                }
            },
        };
        let (frames, ended) = match chunk {
            Some(Ok(bytes)) => (decoder.push(&bytes), false),
            Some(Err(FetchError::Aborted)) if signal.is_some_and(Cancellation::is_aborted) => {
                return Err(RequestFailure::new(ABORTED_TEXT));
            }
            Some(Err(error)) => return Err(RequestFailure::new(error.to_string())),
            None => (decoder.finish(), true),
        };
        for frame in &frames {
            if let Some(event) = event(frame)?
                && matches!(reducer.event(event)?, Progress::Complete)
            {
                return Ok(());
            }
        }
        if ended {
            return Err(RequestFailure::new(UNTERMINATED_TEXT));
        }
    }
}
