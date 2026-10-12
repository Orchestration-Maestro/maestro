//! Prepared content-generation requests shared by the Gemini-style protocols.
//!
//! The records describe the prepared output only; they carry no input schema.
//! Message and tool conversion stay with the parent module.

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use super::{
    Content, FunctionCallingConfigMode, FunctionDeclarations, GoogleThinkingLevel,
    convert_messages, convert_tools,
};
use crate::{Cancellation, Context, DiagnosticErrorInfo, Model, StreamOptions};

/// Parameters of one content-generation request.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentParameters {
    /// Model identifier, exactly as supplied.
    pub model: String,
    /// Converted conversation turns.
    pub contents: Vec<Content>,
    /// Generation settings.
    pub config: GenerateContentConfig,
}

/// Generation settings; absent members are not serialized.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentConfig {
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Output token limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<f64>,
    /// System prompt, present only when nonempty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<String>,
    /// Declared tools, present only when the context has some.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<FunctionDeclarations>>,
    /// Tool-calling mode, present only with declared tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
    /// Selected thinking controls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
    /// The caller's cancellation, serialized as an empty object.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_empty_object"
    )]
    pub abort_signal: Option<Cancellation>,
}

/// Thinking controls; absent members are not serialized.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    /// Whether thoughts are returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
    /// Token budget; -1 asks for a dynamic budget and 0 disables thinking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<f64>,
    /// Level for models that select a level instead of a budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<GoogleThinkingLevel>,
}

/// Tool-calling settings.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    /// Function-calling mode.
    pub function_calling_config: FunctionCallingConfig,
}

/// Function-calling mode wrapper.
#[derive(Clone, Serialize)]
pub struct FunctionCallingConfig {
    /// How the model may use the declared tools.
    pub mode: FunctionCallingConfigMode,
}

/// Emit an empty object whatever the value is; the live handle stays in the record.
fn serialize_empty_object<T, S: Serializer>(_value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_map(Some(0))?.end()
}

/// Assemble request parameters from converted context, common settings and selected controls.
///
/// A tool mode is emitted only with declared tools. An already-aborted signal fails
/// with `Request aborted`; an active one is retained.
pub(crate) fn build_parameters(
    model: &Model,
    context: &Context,
    common: &StreamOptions,
    tool_choice: Option<FunctionCallingConfigMode>,
    thinking_config: Option<ThinkingConfig>,
) -> Result<GenerateContentParameters, DiagnosticErrorInfo> {
    if common.signal.as_ref().is_some_and(Cancellation::is_aborted) {
        return Err(DiagnosticErrorInfo::error("Request aborted"));
    }
    let tools = context
        .tools
        .as_deref()
        .and_then(|tools| convert_tools(tools, false));
    let tool_config = tools.as_ref().and(tool_choice).map(|mode| ToolConfig {
        function_calling_config: FunctionCallingConfig { mode },
    });
    Ok(GenerateContentParameters {
        model: model.id.clone(),
        contents: convert_messages(model, context),
        config: GenerateContentConfig {
            temperature: common.temperature,
            max_output_tokens: common.max_tokens,
            system_instruction: context
                .system_prompt
                .clone()
                .filter(|prompt| !prompt.is_empty()),
            tools,
            tool_config,
            thinking_config,
            abort_signal: common.signal.clone(),
        },
    })
}

/// Thinking controls for an enabled request; an explicit level wins over a budget.
pub(crate) fn enabled_thinking(
    level: Option<GoogleThinkingLevel>,
    budget: Option<f64>,
) -> ThinkingConfig {
    ThinkingConfig {
        include_thoughts: Some(true),
        thinking_budget: budget.filter(|_| level.is_none()),
        thinking_level: level,
    }
}

/// Match `stem`, an optional dotted ASCII version, then `suffix`, anywhere in the lowercased id.
fn matches_family(model_id: &str, stem: &str, suffix: &str) -> bool {
    let id = model_id.to_lowercase();
    id.match_indices(stem).any(|(start, _)| {
        let rest = &id[start + stem.len()..];
        rest.starts_with(suffix)
            || rest.strip_prefix('.').is_some_and(|dotted| {
                let after = dotted.trim_start_matches(|c: char| c.is_ascii_digit());
                after.len() < dotted.len() && after.starts_with(suffix)
            })
    })
}

/// Whether the model id names a fourth-generation Gemma model.
pub(crate) fn is_gemma_4(model_id: &str) -> bool {
    let id = model_id.to_lowercase();
    id.contains("gemma-4") || id.contains("gemma4")
}

/// Whether the model id names a third-generation Gemini pro model.
pub(crate) fn is_gemini_3_pro(model_id: &str) -> bool {
    matches_family(model_id, "gemini-3", "-pro")
}

/// Whether the model id names a third-generation Gemini flash model.
pub(crate) fn is_gemini_3_flash(model_id: &str) -> bool {
    matches_family(model_id, "gemini-3", "-flash")
}
