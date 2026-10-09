//! Reads a response body as server-sent events.

use super::line_decoder::LineDecoder;

#[cfg(test)]
mod tests;

/// One event: its optional name and its data lines joined by line feeds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ServerSentEvent {
    /// The last `event` field of the event, if it had one.
    pub(crate) event: Option<String>,
    /// The `data` fields joined by `\n`.
    pub(crate) data: String,
}

/// Turns lines into events: `event` and `data` fields collect until a blank line dispatches
/// them; comments and every other field are ignored.
#[derive(Default)]
struct SseDecoder {
    /// Name given by the latest `event` field.
    event: Option<String>,
    /// Values of the `data` fields so far.
    data: Vec<String>,
}

impl SseDecoder {
    /// Read one line, returning the event a blank line dispatches. A blank line dispatches
    /// nothing while neither a nonempty event name nor a data field has been seen.
    fn decode(&mut self, line: &str) -> Option<ServerSentEvent> {
        if line.is_empty() {
            if self.event.as_deref().is_none_or(str::is_empty) && self.data.is_empty() {
                return None;
            }
            return Some(ServerSentEvent {
                event: self.event.take(),
                data: std::mem::take(&mut self.data).join("\n"),
            });
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
}

/// Reads server-sent events from body chunks.
///
/// Completed lines are read as each chunk arrives. CR, LF and CRLF end a line; CR ends its
/// line immediately and one following LF is skipped, even across chunks. UTF-8 bytes stay buffered
/// until their line ends. A blank line dispatches the collected event.
#[derive(Default)]
pub(crate) struct SseMessages {
    /// Splits body chunks into lines.
    lines: LineDecoder,
    /// Collects the fields of the lines.
    fields: SseDecoder,
}

impl SseMessages {
    /// Add a body chunk and return the events it completes.
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<ServerSentEvent> {
        let mut events = Vec::new();
        self.read(chunk, &mut events);
        events
    }

    /// End the body: a last line without its ending is read, but an event whose blank line
    /// never arrived is not delivered.
    pub(crate) fn finish(&mut self) -> Vec<ServerSentEvent> {
        let mut events = Vec::new();
        for line in self.lines.flush() {
            events.extend(self.fields.decode(&line));
        }
        events
    }

    /// Decode `bytes` into lines and collect the events they complete.
    fn read(&mut self, bytes: &[u8], events: &mut Vec<ServerSentEvent>) {
        for line in self.lines.decode(bytes) {
            events.extend(self.fields.decode(&line));
        }
    }
}
