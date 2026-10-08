//! Server-sent event framing of a response body.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eventsource_stream::{EventStreamError, Eventsource};
use futures_util::StreamExt;
use serde_json::Value;

use super::events::Reducer;
use crate::Cancellation;
use crate::providers::http::{
    FetchError, HttpBody, Raced, RequestFailure, TextDecoder, race, stream_failure,
};
use crate::providers::json_text::is_truthy;

/// End a body that stops on a lone carriage return with a line feed.
///
/// The event decoder holds a trailing carriage return back until it knows whether a line
/// feed follows; at the end of the body nothing can follow, so the pair is completed here.
fn terminated(body: HttpBody) -> HttpBody {
    let ends_in_return = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&ends_in_return);
    let tail = futures_util::stream::once(async move {
        ends_in_return
            .load(Ordering::Relaxed)
            .then(|| Ok(vec![b'\n']))
    })
    .filter_map(std::future::ready);
    Box::pin(
        body.inspect(move |chunk| {
            if let Some(last) = chunk.as_ref().ok().and_then(|bytes| bytes.last()) {
                seen.store(*last == b'\r', Ordering::Relaxed);
            }
        })
        .chain(tail),
    )
}

/// Read the response body as server-sent events, reducing each chunk until the stream ends.
///
/// The body is decoded as text first (see [`TextDecoder`]), so the event decoder only ever
/// sees valid text. A sequence still cut when the body ends is dropped: it can only belong to
/// a last line that no blank line completes.
///
/// A cancelled signal ends reading quietly; the caller reports the cancellation. A body that
/// reports an abort while the signal is unset is a failure.
///
/// # Errors
/// Fails on a transport or framing error, an event whose data is not JSON or an error payload.
pub(super) async fn consume(
    body: HttpBody,
    signal: Option<&Cancellation>,
    reducer: &mut Reducer,
) -> Result<(), RequestFailure> {
    let mut decoder = TextDecoder::default();
    let text = terminated(body).map(move |chunk| chunk.map(|bytes| decoder.decode(&bytes)));
    let mut events = text.eventsource();
    let mut done = false;
    loop {
        let item = match race(events.next(), None, signal).await {
            Raced::Done(Some(item)) => item,
            Raced::Done(None) | Raced::Cancelled | Raced::TimedOut => return Ok(()),
        };
        let event = match item {
            Ok(event) => event,
            Err(EventStreamError::Transport(FetchError::Aborted))
                if signal.is_some_and(Cancellation::is_aborted) =>
            {
                return Ok(());
            }
            Err(EventStreamError::Transport(error)) => {
                return Err(RequestFailure::new(error.to_string()));
            }
            Err(error) => return Err(RequestFailure::new(error.to_string())),
        };
        if done || event.event.starts_with("thread.") {
            continue;
        }
        if event.data.starts_with("[DONE]") {
            done = true;
            continue;
        }
        let data: Value = serde_json::from_str(&event.data)
            .map_err(|error| RequestFailure::new(error.to_string()))?;
        if let Some(error) = data.get("error").filter(|error| is_truthy(error)) {
            return Err(stream_failure(error));
        }
        reducer.chunk(data);
    }
}
