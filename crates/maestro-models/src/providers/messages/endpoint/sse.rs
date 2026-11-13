//! Server-sent event framing of a message stream, the repair of event data and the point at
//! which the stream is complete.
//!
//! Events come from the shared server-sent event reader, whose line endings a carriage return
//! ends at once, swallowing one line feed that follows it, even in the next chunk. This module
//! selects the framing of a message stream (one byte-order mark at the start, the lines of
//! each event kept for error text) and treats a stream as complete only at its stop event.

use futures_util::StreamExt;
use serde_json::value::RawValue;

use super::ABORTED_TEXT;
use super::events::{Progress, Reducer};
use super::wire::{self, Event};
use crate::Cancellation;
use crate::parameters::repair_json;
use maestro_http::{
    FetchError, HttpBody, Raced, RequestFailure, ServerSentEvent, SseMessages, SseOptions, race,
};

/// Text of the failure of a stream that ends before its stop event.
const UNTERMINATED_TEXT: &str = "The event stream ended before message_stop arrived";
/// The event names whose data is a message event.
const MESSAGE_EVENTS: [&str; 6] = [
    "message_start",
    "message_delta",
    "message_stop",
    "content_block_start",
    "content_block_delta",
    "content_block_stop",
];

/// The framing of a message stream: a byte-order mark only at the start, every line kept for
/// error text, and the event being collected dispatched when the body ends.
const FRAMING: SseOptions = SseOptions {
    stream_start_mark: true,
    raw_lines: true,
    flush_pending: true,
};

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
fn event(frame: &ServerSentEvent) -> Result<Option<Event>, RequestFailure> {
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
    let mut decoder = SseMessages::with_options(FRAMING);
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
