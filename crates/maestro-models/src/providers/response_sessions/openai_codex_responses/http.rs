//! Response-session setup failure policy.

use super::request::diagnostic;
use crate::DiagnosticErrorInfo;
use regex::Regex;
use std::sync::LazyLock;

/// ASCII literals use non-Unicode case matching; the optional separator is one BMP non-line unit.
static RETRYABLE: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    let separator = r"[^\n\r\u{2028}\u{2029}\x{10000}-\x{10FFFF}]?";
    Regex::new(&format!(
        r"(?i-u:rate){separator}(?i-u:limit)|(?i-u:overloaded)|(?i-u:service){separator}(?i-u:unavailable)|(?i-u:upstream){separator}(?i-u:connect)|(?i-u:connection){separator}(?i-u:refused)"
    ))
});

/// Selected statuses always retry; other statuses depend on the authored body pattern.
pub(super) fn is_retryable_error(status: u16, text: &str) -> Result<bool, DiagnosticErrorInfo> {
    if matches!(status, 429 | 500 | 502 | 503 | 504) {
        return Ok(true);
    }
    RETRYABLE
        .as_ref()
        .map(|expression| expression.is_match(text))
        .map_err(|error| diagnostic(error.to_string()))
}

use crate::providers::json_text::{is_truthy, member, raw_json, raw_number};
use serde_json::value::RawValue;

/// Usage codes have the source's non-Unicode case-insensitive matching.
static USAGE_LIMIT: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    Regex::new(r"(?i-u:usage_limit_reached|usage_not_included|rate_limit_exceeded)")
});

/// Selected server text and optional friendly usage text.
pub(super) struct ErrorResponse {
    /// Server message, raw body, status text or fixed fallback.
    pub(super) message: String,
    /// Usage-limit rendering, preferred by invocation.
    pub(super) friendly_message: Option<String>,
}

/// Read a truthy declared string; a wrong type follows the local envelope catch.
fn field(raw: &RawValue, name: &str) -> Result<Option<String>, serde_json::Error> {
    member(raw, name)
        .filter(|value| is_truthy(value))
        .map(|value| serde_json::from_str(value.get()))
        .transpose()
}

/// Read the clock only after a selected usage code and a nonzero reset.
fn friendly(error: &RawValue, now: impl FnOnce() -> f64) -> Result<String, serde_json::Error> {
    let plan = field(error, "plan_type")?.map_or_else(String::new, |plan| {
        format!(" ({} plan)", plan.to_lowercase())
    });
    let when = member(error, "resets_at")
        .and_then(raw_number)
        .filter(|reset| *reset != 0.0)
        .map_or_else(String::new, |reset| {
            let minutes = ((reset * 1000.0 - now()) / 60000.0).round().max(0.0);
            format!(
                " Try again in ~{} min.",
                ryu_js::Buffer::new().format(minutes)
            )
        });
    Ok(format!(
        "You have hit your ChatGPT usage limit{plan}.{when}"
    ))
}

/// Keep the envelope's local catch and the raw/status fallback.
pub(super) fn parse_error_response(
    status: u16,
    raw: &str,
    status_text: &str,
    now: impl FnOnce() -> f64,
) -> ErrorResponse {
    let fallback = if !raw.is_empty() {
        raw
    } else if !status_text.is_empty() {
        status_text
    } else {
        "Request failed"
    };
    let selected = (|| {
        let parsed = raw_json(raw)?;
        let Some(error) = member(parsed, "error").filter(|error| is_truthy(error)) else {
            return Ok(None);
        };
        let code = match field(error, "code")? {
            Some(code) => code,
            None => field(error, "type")?.unwrap_or_default(),
        };
        let usage = status == 429
            || USAGE_LIMIT
                .as_ref()
                .is_ok_and(|pattern| pattern.is_match(&code));
        let friendly_message = if usage {
            Some(friendly(error, now)?)
        } else {
            None
        };
        let message = field(error, "message")?
            .or_else(|| friendly_message.clone())
            .unwrap_or_else(|| fallback.to_owned());
        Ok::<_, serde_json::Error>(Some(ErrorResponse {
            message,
            friendly_message,
        }))
    })();
    selected.ok().flatten().unwrap_or_else(|| ErrorResponse {
        message: fallback.to_owned(),
        friendly_message: None,
    })
}

use super::{CodexError, OpenAICodexResponsesOptions, events::Source, request::PreparedRequest};
use crate::providers::http::{HttpRequest, HttpResponse, Raced, race};
use crate::providers::json_text::compact_json;
use crate::providers::responses::openai_responses_shared::{
    OpenAIResponsesStreamOptions, process_responses_stream,
};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Model, ProviderResponse,
    SharedAssistantMessage,
};
use std::sync::Arc;

/// Send one setup attempt and await its response observation before processing the body.
async fn attempt(
    request: HttpRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions,
) -> Result<HttpResponse, CodexError> {
    let fetch = options
        .common
        .fetch
        .clone()
        .unwrap_or_else(crate::default_fetch);
    let response = match race(fetch(request), None, options.common.signal.as_ref()).await {
        Raced::Done(Ok(response)) => response,
        Raced::Done(Err(error)) => return Err(error.into()),
        Raced::Cancelled | Raced::TimedOut => {
            return Err(CodexError::Transport(diagnostic("Request was aborted")));
        }
    };
    if let Some(hook) = &options.common.on_response {
        hook(
            ProviderResponse {
                status: f64::from(response.status),
                headers: response.headers.clone(),
            },
            Arc::clone(model),
        )
        .await
        .map_err(CodexError::Transport)?;
    }
    Ok(response)
}

/// Announce usable HTTP bodies and delegate content updates without ending the caller's stream.
pub(crate) async fn invoke_sse(
    prepared: &PreparedRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions,
    output: &SharedAssistantMessage,
    events: &AssistantMessageEventStream,
) -> Result<(), CodexError> {
    let body = compact_json(&prepared.body)
        .map_err(|error| CodexError::Transport(diagnostic(error.to_string())))?
        .into_bytes();
    let response = send_response(
        &HttpRequest {
            method: "POST".to_owned(),
            url: prepared.url.clone(),
            headers: prepared.headers.clone(),
            body,
            signal: options.common.signal.clone(),
        },
        model,
        options,
    )
    .await?;
    if matches!(response.status, 204 | 205) {
        return Err(CodexError::Transport(diagnostic("No response body")));
    }
    events.push(AssistantMessageEvent::Start {
        partial: Arc::clone(output),
    });
    let mut source = Source::new(response.body, options.common.signal.clone());
    let view = futures_util::stream::unfold(&mut source, |source| async move {
        source.next().await.map(|event| (event, source))
    });
    let requested = match options.service_tier.as_ref() {
        Some(crate::providers::nullable::Nullable::Value(tier)) => Some(tier.name()),
        _ => None,
    };
    let pricing = |usage: &mut crate::Usage, tier: Option<&str>| {
        super::events::apply_service_tier_pricing(usage, tier, &model.id);
    };
    let stream_options = OpenAIResponsesStreamOptions {
        service_tier: requested,
        resolve_service_tier: Some(&super::events::resolve_codex_service_tier),
        apply_service_tier_pricing: Some(&pricing),
    };
    let result =
        process_responses_stream(Box::pin(view), output, events, model, Some(&stream_options))
            .await;
    result.map_err(|error| source.failure.take().unwrap_or(CodexError::Protocol(error)))
}

use crate::Cancellation;
use crate::providers::http::decode_utf8;
use futures_util::StreamExt;
use std::{future::pending, time::Duration};

/// Read a failed setup response before applying friendly parsing or retry policy.
async fn error_body(
    response: &mut HttpResponse,
    signal: Option<&Cancellation>,
) -> Result<String, CodexError> {
    let mut bytes = Vec::new();
    loop {
        match race(response.body.next(), None, signal).await {
            Raced::Done(Some(Ok(chunk))) => bytes.extend(chunk),
            Raced::Done(None) => return Ok(decode_utf8(&bytes).into_owned()),
            Raced::Done(Some(Err(error))) => return Err(error.into()),
            Raced::Cancelled | Raced::TimedOut => {
                return Err(CodexError::Transport(diagnostic("Request was aborted")));
            }
        }
    }
}

/// Wait with scoped timer and cancellation futures; expiry leaves no registration behind.
async fn wait(attempt: u32, signal: Option<&Cancellation>) -> Result<(), CodexError> {
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(CodexError::Transport(diagnostic("Request was aborted")));
    }
    match race(
        pending::<()>(),
        Some(Duration::from_millis(1000 << attempt)),
        signal,
    )
    .await
    {
        Raced::TimedOut | Raced::Done(()) => Ok(()),
        Raced::Cancelled => Err(CodexError::Transport(diagnostic("Request was aborted"))),
    }
}

/// Caught setup failures use case-sensitive usage exclusion and exact abort identity.
fn caught(error: CodexError) -> Result<CodexError, CodexError> {
    let info = error.diagnostic();
    if info.name.as_deref() == Some("AbortError") || info.message == "Request was aborted" {
        return Err(CodexError::Transport(diagnostic("Request was aborted")));
    }
    if info.message.contains("usage limit") {
        Err(error)
    } else {
        Ok(error)
    }
}

/// Retry only setup work; a classified HTTP failure never re-enters the caught network branch.
async fn send_response(
    request: &HttpRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions,
) -> Result<HttpResponse, CodexError> {
    let signal = options.common.signal.as_ref();
    let mut index = 0;
    loop {
        if signal.is_some_and(Cancellation::is_aborted) {
            return Err(CodexError::Transport(diagnostic("Request was aborted")));
        }
        let setup = async {
            let mut response = attempt(request.clone(), model, options).await?;
            if (200..300).contains(&response.status) {
                return Ok((response, None));
            }
            let text = error_body(&mut response, signal).await?;
            Ok((response, Some(text)))
        }
        .await;
        let (response, text) = match setup {
            Ok(response) => response,
            Err(error) => {
                let error = caught(error)?;
                if index == 3 {
                    return Err(error);
                }
                wait(index, signal).await?;
                index += 1;
                continue;
            }
        };
        let Some(text) = text else {
            return Ok(response);
        };
        if index < 3 && is_retryable_error(response.status, &text).map_err(CodexError::Transport)? {
            wait(index, signal).await?;
            index += 1;
            continue;
        }
        let error = parse_error_response(
            response.status,
            &text,
            &response.status_text,
            crate::records::diagnostics::timestamp_now,
        );
        return Err(CodexError::Transport(diagnostic(
            error.friendly_message.unwrap_or(error.message),
        )));
    }
}
