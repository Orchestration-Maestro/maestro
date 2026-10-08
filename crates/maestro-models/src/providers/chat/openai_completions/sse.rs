//! Server-sent event framing of a response body.

use futures_util::StreamExt;
use serde_json::value::RawValue;

use super::chunk::Chunk;
use super::events::Reducer;
use crate::Cancellation;
use crate::providers::http::{
    FetchError, HttpBody, Raced, RequestFailure, ServerSentEvent, SseMessages, race, stream_failure,
};
use crate::providers::json_text::{is_truthy, object_record};

/// Reduce one event. The first event whose data starts with `[DONE]` ends the data: it and every
/// later event are skipped, as is any event named `thread.*`.
///
/// # Errors
/// Fails on an event whose data is not JSON or whose JSON holds a truthy `error`.
fn reduce(
    event: &ServerSentEvent,
    done: &mut bool,
    reducer: &mut Reducer,
) -> Result<(), RequestFailure> {
    if *done
        || event
            .event
            .as_deref()
            .is_some_and(|name| name.starts_with("thread."))
    {
        return Ok(());
    }
    if event.data.starts_with("[DONE]") {
        *done = true;
        return Ok(());
    }
    let data: &RawValue = serde_json::from_str(&event.data)
        .map_err(|error| RequestFailure::new(error.to_string()))?;
    let Some(chunk) = object_record::<Chunk>(data) else {
        return Ok(());
    };
    if let Some(error) = chunk.error.as_deref().filter(|error| is_truthy(error)) {
        return Err(stream_failure(error));
    }
    reducer.chunk(&chunk);
    Ok(())
}

/// Read the response body as server-sent events, reducing each event until the body ends.
///
/// The body is read as the `OpenAI` client library reads it (see [`SseMessages`]): lines end at
/// `\r\n`, `\r` or `\n`, each line is decoded as text on its own, and an event is delivered
/// when its blank line arrives.
///
/// A cancelled signal ends reading quietly; the caller reports the cancellation. A body that
/// reports an abort while the signal is unset is a failure.
///
/// # Errors
/// Fails on a transport error, an event whose data is not JSON or whose JSON holds a truthy
/// `error`.
pub(super) async fn consume(
    mut body: HttpBody,
    signal: Option<&Cancellation>,
    reducer: &mut Reducer,
) -> Result<(), RequestFailure> {
    let mut messages = SseMessages::default();
    let mut done = false;
    loop {
        let (events, ended) = match race(body.next(), None, signal).await {
            Raced::Done(Some(Ok(bytes))) => (messages.push(&bytes), false),
            Raced::Done(Some(Err(FetchError::Aborted)))
                if signal.is_some_and(Cancellation::is_aborted) =>
            {
                return Ok(());
            }
            Raced::Done(Some(Err(error))) => return Err(RequestFailure::new(error.to_string())),
            Raced::Done(None) => (messages.finish(), true),
            Raced::Cancelled | Raced::TimedOut => return Ok(()),
        };
        for event in &events {
            reduce(event, &mut done, reducer)?;
        }
        if ended {
            return Ok(());
        }
    }
}
