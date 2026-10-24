//! Endpoint response preparation and event framing.

use super::diagnostic;
use crate::parameters::json_parse::whitespace;
use crate::providers::json_text::{is_truthy, member, raw_json};
use crate::providers::transport::{
    HttpBody, HttpResponse, Raced, RequestFailure, ServerSentEvent, SseMessages, decode_utf8, race,
    sdk_stream_failure,
};
use crate::{Cancellation, DiagnosticErrorInfo, FetchError, ProviderResponse};
use futures_util::StreamExt;
pub(in crate::providers::responses_api) use maestro_json::is_truthy as streaming;
use std::collections::VecDeque;

/// Framing state for one body.
pub(in crate::providers::responses_api) struct Source {
    /// Underlying transport body.
    pub(in crate::providers::responses_api) body: HttpBody,
    /// Shared frame reader.
    pub(in crate::providers::responses_api) messages: SseMessages,
    /// Completed event data awaiting reduction.
    pub(in crate::providers::responses_api) pending: VecDeque<ServerSentEvent>,
    /// Whether the body ended.
    pub(in crate::providers::responses_api) ended: bool,
    /// Whether a DONE prefix has suppressed further parsing.
    pub(in crate::providers::responses_api) done: bool,
    /// Cancellation of body consumption.
    pub(in crate::providers::responses_api) signal: Option<Cancellation>,
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
    pub(in crate::providers::responses_api) async fn next(
        &mut self,
    ) -> Option<Result<String, DiagnosticErrorInfo>> {
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
                Raced::Done(Some(Err(FetchError::Aborted)))
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
pub(in crate::providers::responses_api) enum Prepared {
    /// Streaming bodies remain unread until after the response hook.
    Stream(HttpBody),
    /// A consumed non-event response cannot feed the event reducer.
    Nonstream,
}

/// Consume non-event responses before exposing them to the response hook.
pub(in crate::providers::responses_api) async fn prepare(
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
        .map(|s| s.trim_matches(whitespace));
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
