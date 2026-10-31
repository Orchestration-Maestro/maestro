//! Account selection from request credentials.

use crate::DiagnosticErrorInfo;
use crate::providers::base64_text::FORGIVING;
use crate::providers::json_text::{member, raw_json};
use base64::Engine as _;

/// Read the account claim without signature verification.
pub(super) fn extract_account_id(token: &str) -> Result<String, DiagnosticErrorInfo> {
    let failure = || DiagnosticErrorInfo::error("Failed to extract accountId from token");
    let parts: Vec<_> = token.split('.').collect();
    let [_, payload, _] = parts.as_slice() else {
        return Err(failure());
    };
    let bytes = FORGIVING.decode(payload).map_err(|_| failure())?;
    let text: String = bytes.into_iter().map(char::from).collect();
    let raw = raw_json(&text).map_err(|_| failure())?;
    let account = member(raw, "https://api.openai.com/auth")
        .and_then(|claims| member(claims, "chatgpt_account_id"))
        .and_then(|account| serde_json::from_str::<String>(account.get()).ok())
        .filter(|account| !account.is_empty());
    account.ok_or_else(failure)
}

use super::{OpenAICodexResponsesOptions, OpenAICodexTextVerbosity};
use crate::arguments::json_parse::is_blank;
use crate::providers::responses::openai_responses_shared::{
    ConvertResponsesMessagesOptions, ConvertResponsesToolsOptions,
    messages::{convert_responses_messages, convert_responses_tools},
};
use crate::{Context, Model};
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};

/// Prepared payload, endpoint and effective headers.
pub(crate) struct PreparedRequest {
    /// Retained wire document.
    pub(crate) body: Value,
    /// Authored endpoint spelling.
    pub(crate) url: String,
    /// Validated effective headers.
    pub(crate) headers: IndexMap<String, String>,
}

/// A failure with its authored diagnostic identity.
pub(super) fn diagnostic(message: impl Into<String>) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(message)
}

/// Ordered serialization-only response-session body.
#[derive(Serialize)]
struct RequestBody<'a> {
    /// Selected model.
    model: &'a str,
    /// Disable storage.
    store: bool,
    /// Request streaming.
    stream: bool,
    /// System instructions.
    instructions: &'a str,
    /// Shared converted history.
    input: Vec<Value>,
    /// Requested verbosity.
    text: Text,
    /// Replayable reasoning material.
    include: [&'a str; 1],
    /// Supplied session, including empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<&'a str>,
    /// Automatic tool selection.
    tool_choice: &'a str,
    /// Allow parallel tool calls.
    parallel_tool_calls: bool,
    /// Optional temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Optional nullable service tier.
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<&'a super::Nullable<super::OpenAIResponsesServiceTier>>,
    /// Nonempty tool declarations.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    /// Explicit requested effort and default summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning<'a>>,
}

/// Effective descriptor-mapped effort.
#[derive(Serialize)]
struct Reasoning<'a> {
    /// Wire spelling, including an empty descriptor mapping.
    effort: &'a str,
    /// Summary selected when reasoning was requested.
    summary: super::OpenAICodexReasoningSummary,
}

/// Keep explicit effort even when the descriptor does not advertise reasoning.
fn reasoning<'a, E: crate::ProviderExecution>(
    model: &'a Model,
    options: &OpenAICodexResponsesOptions<E>,
) -> Option<Reasoning<'a>> {
    use crate::ModelThinkingLevel as Level;
    let level = options.reasoning_effort.as_ref()?;
    let fallback = match level {
        Level::Off => "none",
        Level::Minimal => "minimal",
        Level::Low => "low",
        Level::Medium => "medium",
        Level::High => "high",
        Level::Xhigh => "xhigh",
    };
    let effort = model
        .thinking_level_map
        .as_ref()
        .and_then(|map| map.get(level))
        .and_then(Option::as_deref)
        .unwrap_or(fallback);
    Some(Reasoning {
        effort,
        summary: options
            .reasoning_summary
            .unwrap_or(super::OpenAICodexReasoningSummary::Auto),
    })
}

/// Text output preferences.
#[derive(Serialize)]
struct Text {
    /// Detail selection.
    verbosity: OpenAICodexTextVerbosity,
}

/// Convert request data before the arbitrary JSON payload-hook seam.
fn build_request_body<E: crate::ProviderExecution>(
    model: &Model,
    context: &Context,
    options: &OpenAICodexResponsesOptions<E>,
) -> Result<Value, DiagnosticErrorInfo> {
    let input = convert_responses_messages(
        model,
        context,
        &HashSet::from([
            "openai".to_owned(),
            "openai-codex".to_owned(),
            "opencode".to_owned(),
        ]),
        Some(&ConvertResponsesMessagesOptions {
            include_system_prompt: false,
        }),
    )?;
    serde_json::to_value(RequestBody {
        model: &model.id,
        store: false,
        stream: true,
        instructions: context
            .system_prompt
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("You are a helpful assistant."),
        input,
        text: Text {
            verbosity: options
                .text_verbosity
                .unwrap_or(OpenAICodexTextVerbosity::Low),
        },
        include: ["reasoning.encrypted_content"],
        prompt_cache_key: options.common.session_id.as_deref(),
        tool_choice: "auto",
        parallel_tool_calls: true,
        temperature: options.common.temperature,
        service_tier: options.service_tier.as_ref(),
        reasoning: reasoning(model, options),
        tools: context
            .tools
            .as_deref()
            .filter(|tools| !tools.is_empty())
            .map(|tools| {
                convert_responses_tools(tools, Some(&ConvertResponsesToolsOptions { strict: None }))
            }),
    })
    .map_err(|error| diagnostic(error.to_string()))
}

/// Resolve credentials, convert and edit the body once, then validate headers.
pub(crate) async fn prepare_request<E: crate::ProviderExecution>(
    model: &Arc<Model>,
    context: &Context,
    options: &OpenAICodexResponsesOptions<E>,
    user_agent: &str,
) -> Result<PreparedRequest, super::CodexError> {
    let key = options
        .common
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .or_else(|| crate::get_env_api_key(&model.provider))
        .ok_or_else(|| diagnostic(format!("No API key for provider: {}", model.provider)))?;
    let account = extract_account_id(&key)?;
    let mut body = build_request_body(model, context, options)?;
    if let Some(hook) = &options.common.on_payload {
        body = hook(
            crate::PayloadHookInput {
                payload: body,
                signal: None,
            },
            Arc::clone(model),
        )
        .await?
        .payload;
    }
    Ok(PreparedRequest {
        body,
        url: resolve_codex_url(&model.base_url),
        headers: super::headers::build_sse_headers(
            model,
            &options.common,
            &account,
            &key,
            user_agent,
        )?,
    })
}

/// Resolve an authored base path without rewriting its nonempty whitespace or URL suffixes.
pub(super) fn resolve_codex_url(base: &str) -> String {
    let base = if is_blank(base) {
        "https://chatgpt.com/backend-api"
    } else {
        base
    };
    let base = base.trim_end_matches('/');
    if base.ends_with("/codex/responses") {
        base.to_owned()
    } else if base.ends_with("/codex") {
        format!("{base}/responses")
    } else {
        format!("{base}/codex/responses")
    }
}
