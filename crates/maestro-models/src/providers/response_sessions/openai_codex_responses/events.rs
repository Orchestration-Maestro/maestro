//! Provider-specific SSE selection before the shared response reducer.

use super::{CodexError, OpenAICodexResponsesOptions, request::diagnostic};
use crate::arguments::json_parse::whitespace;
use crate::providers::http::{HttpBody, Raced, ServerSentEvent, SseMessages, race};
use crate::providers::json_text::{compact_raw, member, raw_json};
use crate::providers::nullable::Nullable;
use crate::providers::responses::openai_responses_shared::{
    OpenAIResponsesStreamOptions, process_responses_stream,
};
use crate::{
    AssistantMessageEventStream, Cancellation, DiagnosticErrorInfo, Model, SharedAssistantMessage,
    Usage,
};
use futures_util::StreamExt;
use indexmap::IndexMap;
use serde_json::value::RawValue;
use std::collections::VecDeque;

/// Framing state and typed failure of one borrowed reducer source.
pub(super) struct Source {
    /// Transport body owned until reduction ends.
    body: HttpBody,
    /// Shared line and event framing.
    messages: SseMessages,
    /// Completed frames awaiting provider selection.
    pending: VecDeque<ServerSentEvent>,
    /// Stop immediately after a terminal frame or body EOF.
    ended: bool,
    /// Caller cancellation.
    signal: Option<Cancellation>,
    /// Failure category recovered after the reducer sees its diagnostic view.
    pub(super) failure: Option<CodexError>,
}
impl Source {
    /// Begin consuming one transport body.
    pub(super) fn new(body: HttpBody, signal: Option<Cancellation>) -> Self {
        Self {
            body,
            messages: SseMessages::default(),
            pending: VecDeque::new(),
            ended: false,
            signal,
            failure: None,
        }
    }

    /// Keep the category while exposing the reducer's existing diagnostic item.
    fn failed(&mut self, error: CodexError) -> DiagnosticErrorInfo {
        let diagnostic = error.diagnostic().clone();
        self.failure = Some(error);
        self.ended = true;
        diagnostic
    }

    /// Select the next event, cutting the body after a terminal event.
    pub(super) async fn next(&mut self) -> Option<Result<String, DiagnosticErrorInfo>> {
        loop {
            if self.ended {
                return None;
            }
            if self.signal.as_ref().is_some_and(Cancellation::is_aborted) {
                return Some(Err(
                    self.failed(CodexError::Transport(diagnostic("Request was aborted")))
                ));
            }
            if let Some(event) = self.pending.pop_front() {
                match self.frame(&event.data) {
                    Ok(Some(text)) => return Some(Ok(text)),
                    Ok(None) => continue,
                    Err(error) => return Some(Err(self.failed(error))),
                }
            }
            let frames = match race(self.body.next(), None, self.signal.as_ref()).await {
                Raced::Done(Some(Ok(bytes))) => self.messages.push(&bytes),
                Raced::Done(None) => {
                    self.ended = true;
                    return None;
                }
                Raced::Done(Some(Err(error))) => return Some(Err(self.failed(error.into()))),
                Raced::Cancelled | Raced::TimedOut => {
                    return Some(Err(
                        self.failed(CodexError::Transport(diagnostic("Request was aborted")))
                    ));
                }
            };
            self.pending.extend(frames);
        }
    }

    /// Trim each data field, ignore DONE, then normalize only selected event members.
    fn frame(&mut self, data: &str) -> Result<Option<String>, CodexError> {
        let joined = data
            .split('\n')
            .map(|line| line.trim_matches(whitespace))
            .collect::<Vec<_>>()
            .join("\n");
        let text = joined.trim_matches(whitespace);
        if text.is_empty() || text == "[DONE]" {
            return Ok(None);
        }
        let raw = raw_json(text).map_err(|error| {
            let mut error = diagnostic(format!("Invalid Codex SSE JSON: {error}"));
            error.name = Some("CodexProtocolError".to_owned());
            CodexError::Protocol(error)
        })?;
        match map_codex_event(raw)? {
            Some((text, terminal)) => {
                self.ended = terminal;
                Ok(Some(text))
            }
            None => Ok(None),
        }
    }
}

/// Select the retained event text and whether it ends the response; untyped events yield none.
pub(super) fn map_codex_event(raw: &RawValue) -> Result<Option<(String, bool)>, CodexError> {
    let Some(kind) = string(raw, "type").filter(|kind| !kind.is_empty()) else {
        return Ok(None);
    };
    match kind.as_str() {
        "error" | "response.failed" => Err(api_error(raw, &kind)?),
        "response.done" | "response.completed" | "response.incomplete" => terminal(raw)
            .map(|text| Some((text, true)))
            .map_err(|error| CodexError::Protocol(diagnostic(error.to_string()))),
        _ => Ok(Some((raw.get().to_owned(), false))),
    }
}

/// Select a declared string without materializing unread members.
fn string(raw: &RawValue, name: &str) -> Option<String> {
    member(raw, name).and_then(|value| serde_json::from_str(value.get()).ok())
}

/// Preserve the server's API diagnostic code and message selection.
fn api_error(raw: &RawValue, kind: &str) -> Result<CodexError, CodexError> {
    let (message, code) = if kind == "error" {
        let code = string(raw, "code").filter(|code| !code.is_empty());
        let detail = string(raw, "message")
            .filter(|message| !message.is_empty())
            .or_else(|| code.clone());
        let detail = match detail {
            Some(detail) => detail,
            None => compact_raw(raw)
                .map_err(|error| CodexError::Protocol(diagnostic(error.to_string())))?,
        };
        (format!("Codex error: {detail}"), code)
    } else {
        let nested = member(raw, "response").and_then(|response| member(response, "error"));
        let code = nested.and_then(|error| string(error, "code"));
        let message = nested
            .and_then(|error| string(error, "message"))
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| "Codex response failed".to_owned());
        (message, code)
    };
    Ok(CodexError::Api(DiagnosticErrorInfo {
        name: Some("CodexApiError".to_owned()),
        message,
        code: code.map(crate::DiagnosticCode::Text),
        stack: None,
    }))
}

/// Preserve admitted response statuses and omit every other selected value.
pub(super) fn normalize_codex_status(raw: Option<&RawValue>) -> Option<String> {
    let status: String = serde_json::from_str(raw?.get()).ok()?;
    matches!(
        status.as_str(),
        "completed" | "incomplete" | "failed" | "cancelled" | "queued" | "in_progress"
    )
    .then_some(status)
}

/// Normalize terminal tags and response status while keeping other members raw.
fn terminal(raw: &RawValue) -> Result<String, serde_json::Error> {
    let mut fields: IndexMap<String, &RawValue> = serde_json::from_str(raw.get())?;
    fields.insert("type".to_owned(), raw_json(r#""response.completed""#)?);
    let mut response_text = None;
    if let Some(response) = fields
        .get("response")
        .filter(|response| response.get().starts_with('{'))
    {
        let mut response: IndexMap<String, &RawValue> = serde_json::from_str(response.get())?;
        let status = normalize_codex_status(response.get("status").copied());
        if status.is_none() {
            response.shift_remove("status");
        }
        response_text = Some(serde_json::to_string(&response)?);
    }
    if let Some(text) = &response_text {
        fields.insert("response".to_owned(), raw_json(text)?);
    }
    serde_json::to_string(&fields)
}

/// An echoed default must not conceal a requested flex or priority tier.
pub(super) fn resolve_codex_service_tier(
    response: Option<&str>,
    request: Option<&str>,
) -> Option<String> {
    if response == Some("default") && matches!(request, Some("flex" | "priority")) {
        request.map(str::to_owned)
    } else {
        response.or(request).map(str::to_owned)
    }
}

pub(super) use crate::providers::responses::openai_responses::price as apply_service_tier_pricing;

/// Reduce selected events into `output`, pricing the echoed tier against the requested one.
pub(super) async fn reduce<S>(
    events: S,
    model: &Model,
    options: &OpenAICodexResponsesOptions,
    output: &SharedAssistantMessage,
    stream: &AssistantMessageEventStream,
) -> Result<(), DiagnosticErrorInfo>
where
    S: futures_core::Stream<Item = Result<String, DiagnosticErrorInfo>> + Unpin,
{
    let requested = match options.service_tier.as_ref() {
        Some(Nullable::Value(tier)) => Some(tier.name()),
        _ => None,
    };
    let pricing = |usage: &mut Usage, tier: Option<&str>| {
        apply_service_tier_pricing(usage, tier, &model.id);
    };
    let stream_options = OpenAIResponsesStreamOptions {
        service_tier: requested,
        resolve_service_tier: Some(&resolve_codex_service_tier),
        apply_service_tier_pricing: Some(&pricing),
    };
    process_responses_stream(events, output, stream, model, Some(&stream_options)).await
}
