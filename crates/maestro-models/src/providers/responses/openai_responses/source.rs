//! Endpoint response preparation and event framing.

use super::diagnostic;
use crate::providers::http::{
    HttpBody, HttpResponse, Raced, RequestFailure, ServerSentEvent, SseMessages, decode_utf8, race,
    sdk_stream_failure,
};
use crate::providers::json_text::{is_truthy, member, raw_json};
use crate::{Cancellation, DiagnosticErrorInfo, ProviderResponse};
use futures_util::StreamExt;
use std::collections::VecDeque;

/// Framing state for one body.
pub(super) struct Source {
    /// Underlying transport body.
    pub(super) body: HttpBody,
    /// Shared frame reader.
    pub(super) messages: SseMessages,
    /// Completed event data awaiting reduction.
    pub(super) pending: VecDeque<ServerSentEvent>,
    /// Whether the body ended.
    pub(super) ended: bool,
    /// Whether a DONE prefix has suppressed further parsing.
    pub(super) done: bool,
    /// Cancellation of body consumption.
    pub(super) signal: Option<Cancellation>,
}
impl Source {
    /// Validate one frame; DONE suppresses parsing while the body continues to drain.
    fn frame(&mut self, event: ServerSentEvent) -> Result<Option<String>, DiagnosticErrorInfo> {
        if self.done || event.data.starts_with("[DONE]") {
            self.done = true;
            return Ok(None);
        }
        let parsed = raw_json(&event.data).map_err(|error| diagnostic(error.to_string()))?;
        if event
            .event
            .as_deref()
            .is_some_and(|name| name.starts_with("thread."))
        {
            return Ok(None);
        }
        if let Some(error) = member(parsed, "error").filter(|v| is_truthy(v)) {
            return Err(diagnostic(
                sdk_stream_failure(error)
                    .map_err(|error| diagnostic(error.to_string()))?
                    .into_text(),
            ));
        }
        Ok(Some(event.data))
    }

    /// Read the next framed event or source failure.
    pub(super) async fn next(&mut self) -> Option<Result<String, DiagnosticErrorInfo>> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                match self.frame(event) {
                    Ok(None) => continue,
                    Ok(Some(data)) => return Some(Ok(data)),
                    Err(error) => return Some(Err(error)),
                }
            }
            if self.ended {
                return None;
            }
            let frames = match race(self.body.next(), None, self.signal.as_ref()).await {
                Raced::Done(Some(Ok(bytes))) => self.messages.push(&bytes),
                Raced::Done(None) => {
                    self.ended = true;
                    self.messages.finish()
                }
                Raced::Done(Some(Err(_)))
                    if self.signal.as_ref().is_some_and(Cancellation::is_aborted) =>
                {
                    return Some(Err(diagnostic("Request was aborted")));
                }
                Raced::Done(Some(Err(error))) => return Some(Err(diagnostic(error.to_string()))),
                Raced::Cancelled | Raced::TimedOut => {
                    return Some(Err(diagnostic("Request was aborted")));
                }
            };
            self.pending.extend(frames);
        }
    }
}

/// Response ready for the invocation's processing branch.
pub(super) enum Prepared {
    /// Streaming bodies remain unread until after the response hook.
    Stream(HttpBody),
    /// A consumed non-event response cannot feed the event reducer.
    Nonstream,
}

/// The edited stream flag's truthiness.
pub(super) fn streaming(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(number) => number.as_f64() != Some(0.0),
        serde_json::Value::String(text) => !text.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

/// Consume non-event responses before exposing them to the response hook.
pub(super) async fn prepare(
    response: HttpResponse,
    streaming: bool,
    signal: Option<&Cancellation>,
) -> Result<(Prepared, ProviderResponse), RequestFailure> {
    let HttpResponse {
        status,
        headers,
        mut body,
        ..
    } = response;
    let observation = ProviderResponse {
        status: f64::from(status),
        headers,
    };
    if streaming {
        return Ok((Prepared::Stream(body), observation));
    }
    if status == 204 {
        return Err(RequestFailure::new("Response body is absent"));
    }
    let media_type = observation
        .headers
        .get("content-type")
        .and_then(|s| s.split(';').next())
        .map(str::trim);
    let json = media_type.is_some_and(|s| s.contains("application/json") || s.ends_with("+json"));
    if json
        && observation
            .headers
            .get("content-length")
            .is_some_and(|s| s == "0")
    {
        return Err(RequestFailure::new("Response body is absent"));
    }
    let mut bytes = Vec::new();
    loop {
        match race(body.next(), None, signal).await {
            Raced::Done(Some(Ok(chunk))) => bytes.extend(chunk),
            Raced::Done(Some(Err(error))) => return Err(RequestFailure::new(error.to_string())),
            Raced::Done(None) => break,
            Raced::Cancelled | Raced::TimedOut => return Err(RequestFailure::aborted()),
        }
    }
    let text = decode_utf8(&bytes);
    if !json {
        return Err(RequestFailure::new("Response body is not an object"));
    }
    let parsed = raw_json(&text).map_err(|error| RequestFailure::new(error.to_string()))?;
    if !matches!(parsed.get().as_bytes().first(), Some(b'{' | b'[')) {
        return Err(RequestFailure::new("Response body is not an object"));
    }
    Ok((Prepared::Nonstream, observation))
}
