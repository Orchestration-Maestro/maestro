//! Private conversation request preparation.

pub(super) mod errors;
mod messages;
pub(super) mod tool_ids;
pub(super) mod transport;

use super::MistralOptions;
use crate::{Context, DiagnosticErrorInfo, Model};
use serde::Serialize;
use serde_json::Value;

/// Trim content according to the shared authored-whitespace policy.
fn trim(text: &str) -> &str {
    text.trim_matches(crate::arguments::json_parse::whitespace)
}

/// Keep a native failure's message.
fn native_error(error: impl std::fmt::Display) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: None,
        message: error.to_string(),
        stack: None,
        code: None,
    }
}

/// Borrowed callback fields, distinct from the post-hook wire records.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payload<'a> {
    /// Destination model.
    model: &'a str,
    /// Request streaming response.
    stream: bool,
    /// Projected messages.
    messages: Vec<messages::ChatMessage<'a>>,
    /// Nonempty tool declarations.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<FunctionTool<'a>>,
    /// Optional raw temperature, including zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Optional raw token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<f64>,
    /// Raw tool selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a super::MistralToolChoice>,
    /// Raw or selected prompt mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_mode: Option<super::MistralPromptMode>,
    /// Open model-selected effort.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

/// Function declaration wrapper.
#[derive(Serialize)]
struct FunctionTool<'a> {
    /// Function discriminator.
    r#type: &'static str,
    /// Borrowed declaration.
    function: ToolFunction<'a>,
}

/// Function schema in callback spelling.
#[derive(Serialize)]
struct ToolFunction<'a> {
    /// Function name.
    name: &'a str,
    /// Function description.
    description: &'a str,
    /// Arbitrary schema, checked after the hook.
    parameters: &'a Value,
    /// Non-strict declaration.
    strict: bool,
}

/// Project history and serialize selected raw settings for the hook.
pub(super) fn build_chat_payload(
    model: &Model,
    context: &Context,
    options: &MistralOptions,
    mapped_effort: Option<&str>,
) -> Result<Value, DiagnosticErrorInfo> {
    for value in [options.common.temperature, options.common.max_tokens]
        .into_iter()
        .flatten()
    {
        if !value.is_finite() {
            return Err(native_error(
                "Input validation failed: expected a finite number",
            ));
        }
    }
    let projected = messages::project(context, model);
    let tools = context
        .tools
        .iter()
        .flatten()
        .map(|tool| FunctionTool {
            r#type: "function",
            function: ToolFunction {
                name: &tool.name,
                description: &tool.description,
                parameters: &tool.parameters,
                strict: false,
            },
        })
        .collect();
    let effort = mapped_effort
        .or(options.reasoning_effort.map(|effort| match effort {
            super::MistralReasoningEffort::None => "none",
            super::MistralReasoningEffort::High => "high",
        }))
        .filter(|effort| !effort.is_empty());
    serde_json::to_value(Payload {
        model: &model.id,
        stream: true,
        messages: messages::convert(&projected, context.system_prompt.as_deref())?,
        tools,
        temperature: options.common.temperature,
        max_tokens: options.common.max_tokens,
        tool_choice: options.tool_choice.as_ref(),
        prompt_mode: options.prompt_mode,
        reasoning_effort: effort,
    })
    .map_err(native_error)
}

/// Select the effective simple controls without closing model mapping strings.
pub(super) fn simple_options(
    model: &Model,
    options: &crate::SimpleStreamOptions,
) -> (MistralOptions, Option<String>) {
    let mut raw = MistralOptions {
        common: crate::build_base_options(model, Some(options), None),
        ..Default::default()
    };
    let Some(level) = options.reasoning.map(crate::ModelThinkingLevel::from) else {
        return (raw, None);
    };
    let level = crate::clamp_thinking_level(model, level);
    if !model.reasoning || level == crate::ModelThinkingLevel::Off {
        return (raw, None);
    }
    if matches!(
        model.id.as_str(),
        "mistral-small-2603" | "mistral-small-latest" | "mistral-medium-3.5"
    ) {
        let effort = model
            .thinking_level_map
            .as_ref()
            .and_then(|map| map.get(&level))
            .and_then(Option::as_deref)
            .unwrap_or("high")
            .to_owned();
        (raw, Some(effort))
    } else {
        raw.prompt_mode = Some(super::MistralPromptMode::Reasoning);
        (raw, None)
    }
}

/// Merge exact-case authored layers before the HTTP normalization phase.
pub(super) fn authored_headers(
    model: &Model,
    options: &crate::StreamOptions,
) -> indexmap::IndexMap<String, String> {
    let mut headers = model.headers.clone().unwrap_or_default();
    if let Some(layer) = &options.headers {
        headers.extend(
            layer
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }
    if let Some(session) = options.session_id.as_ref().filter(|id| !id.is_empty())
        && headers.get("x-affinity").is_none_or(String::is_empty)
    {
        headers.insert("x-affinity".into(), session.clone());
    }
    headers
}

/// Prepared native request and the selected replacement transport.
pub(super) struct Prepared {
    /// Fully assembled request.
    request: crate::HttpRequest,
    /// One-attempt client.
    fetch: crate::Fetch,
}

/// Resolve credentials and URL before the hook, then encode its replacement.
pub(super) async fn prepare(
    model: std::sync::Arc<Model>,
    context: &Context,
    options: &MistralOptions,
    mapped_effort: Option<&str>,
) -> Result<Prepared, crate::providers::http::RequestFailure> {
    use crate::providers::http::RequestFailure;
    let key = options
        .common
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .map(std::borrow::Cow::Borrowed)
        .or_else(|| crate::get_env_api_key(model.provider.as_str()).map(std::borrow::Cow::Owned))
        .ok_or_else(|| {
            RequestFailure::new(format!("No API key for provider: {}", model.provider))
        })?;
    let base = transport::base_url(&model.base_url)?;
    let mut payload = build_chat_payload(&model, context, options, mapped_effort)?;
    if let Some(hook) = &options.common.on_payload {
        payload = hook(payload, std::sync::Arc::clone(&model)).await?;
    }
    let wire = super::wire::encode_payload(&payload)?;
    let body = crate::providers::json_text::compact_json(&wire)
        .map_err(|error| RequestFailure::new(error.to_string()))?
        .into_bytes();
    let headers = transport::headers(&key, authored_headers(&model, &options.common))?;
    let url = transport::endpoint(&base)?;
    Ok(Prepared {
        request: crate::HttpRequest {
            method: "POST".into(),
            url,
            headers,
            body,
            signal: options.common.signal.clone(),
        },
        fetch: options
            .common
            .fetch
            .clone()
            .unwrap_or_else(crate::default_fetch),
    })
}

/// Send an already prepared request under this provider's one-attempt policy.
pub(super) async fn send(
    prepared: Prepared,
) -> Result<crate::HttpBody, crate::providers::http::RequestFailure> {
    transport::send(prepared).await
}
