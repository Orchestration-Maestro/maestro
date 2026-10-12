//! Content-generation request preparation: raw parameters and simple-option selection.
//!
//! Neither operation sends a request; see the content-generation guide for the full flow.

use std::borrow::Cow;

use super::google_shared::{
    FunctionCallingConfigMode, GenerateContentParameters, GoogleThinkingLevel, ThinkingConfig,
    build_parameters, enabled_thinking, is_gemini_3_flash, is_gemini_3_pro, is_gemma_4,
};
use crate::options::resolve_api_key;
use crate::{
    Context, DiagnosticErrorInfo, Model, ModelThinkingLevel, SimpleStreamOptions, StreamOptions,
    ThinkingBudgets, ThinkingLevel, build_base_options, clamp_reasoning, clamp_thinking_level,
};

/// Content-generation options: common settings, tool choice and optional raw thinking.
#[derive(Clone, Default)]
pub struct GoogleOptions {
    /// Settings shared by every provider.
    pub common: StreamOptions,
    /// Tool-calling mode, used only when the context declares tools.
    pub tool_choice: Option<FunctionCallingConfigMode>,
    /// Raw thinking request; absent leaves thinking unspecified.
    pub thinking: Option<GoogleThinking>,
}

/// Raw thinking request. An explicit level wins over a budget.
#[derive(Clone, Default)]
pub struct GoogleThinking {
    /// Whether thinking is requested.
    pub enabled: bool,
    /// Token budget; -1 asks for a dynamic budget and 0 disables thinking.
    pub budget_tokens: Option<f64>,
    /// Level for models that select a level instead of a budget.
    pub level: Option<GoogleThinkingLevel>,
}

/// Prepare request parameters without resolving a key, calling hooks or sending anything.
///
/// Thinking controls are emitted only for a reasoning model: an enabled request carries its
/// level or budget with thoughts included, a disabled one selects the family's lowest setting.
///
/// # Errors
/// Fails with `Request aborted` when the supplied signal is already aborted.
pub fn build_params(
    model: &Model,
    context: &Context,
    options: Option<&GoogleOptions>,
) -> Result<GenerateContentParameters, DiagnosticErrorInfo> {
    let default = GoogleOptions::default();
    let options = options.unwrap_or(&default);
    let thinking = options
        .thinking
        .as_ref()
        .filter(|_| model.reasoning)
        .map(|thinking| {
            if thinking.enabled {
                enabled_thinking(thinking.level, thinking.budget_tokens)
            } else {
                disabled_thinking(&model.id)
            }
        });
    build_parameters(
        model,
        context,
        &options.common,
        options.tool_choice,
        thinking,
    )
}

/// Select raw options from simple ones: key, common settings and family thinking.
///
/// Missing reasoning requests disabled thinking. An effort is clamped to the model's supported
/// levels first; extra-high and off both select high. Level-based families map the effort to a
/// level and ignore custom budgets; other models use the selected budget slot or the family
/// default. The context and cancellation are not inspected.
///
/// # Errors
/// Fails with `No API key for provider: <provider>` when neither a nonempty supplied key nor
/// the environment provides one.
pub fn build_simple_options(
    model: &Model,
    options: Option<&SimpleStreamOptions>,
) -> Result<GoogleOptions, DiagnosticErrorInfo> {
    let key = resolve_api_key(
        &model.provider,
        options
            .and_then(|options| options.common.api_key.as_deref())
            .map(Cow::Borrowed),
    )
    .ok_or_else(|| {
        DiagnosticErrorInfo::error(format!("No API key for provider: {}", model.provider))
    })?;
    let thinking = match options.and_then(|options| options.reasoning) {
        None => GoogleThinking::default(),
        Some(reasoning) => {
            let budgets = options.and_then(|options| options.thinking_budgets.as_ref());
            enabled_for_effort(model, effort(model, reasoning), budgets)
        }
    };
    Ok(GoogleOptions {
        common: build_base_options(model, options, Some(&key)),
        tool_choice: None,
        thinking: Some(thinking),
    })
}

/// Clamp a requested level to the model's support; off and extra-high select high.
fn effort(model: &Model, requested: ThinkingLevel) -> ThinkingLevel {
    let supported = match clamp_thinking_level(model, requested.into()) {
        ModelThinkingLevel::Minimal => ThinkingLevel::Minimal,
        ModelThinkingLevel::Low => ThinkingLevel::Low,
        ModelThinkingLevel::Medium => ThinkingLevel::Medium,
        ModelThinkingLevel::Off | ModelThinkingLevel::High => ThinkingLevel::High,
        ModelThinkingLevel::Xhigh => ThinkingLevel::Xhigh,
    };
    clamp_reasoning(Some(supported)).unwrap_or(ThinkingLevel::High)
}

/// Enabled thinking for an effort: a level for level-based families, else a budget.
fn enabled_for_effort(
    model: &Model,
    effort: ThinkingLevel,
    budgets: Option<&ThinkingBudgets>,
) -> GoogleThinking {
    let level_based =
        is_gemini_3_pro(&model.id) || is_gemini_3_flash(&model.id) || is_gemma_4(&model.id);
    if level_based {
        GoogleThinking {
            enabled: true,
            budget_tokens: None,
            level: Some(thinking_level(effort, &model.id)),
        }
    } else {
        GoogleThinking {
            enabled: true,
            budget_tokens: Some(budget(&model.id, effort, budgets)),
            level: None,
        }
    }
}

/// Lowest thinking setting a family accepts, without returning thoughts.
fn disabled_thinking(model_id: &str) -> ThinkingConfig {
    let level = if is_gemini_3_pro(model_id) {
        Some(GoogleThinkingLevel::Low)
    } else if is_gemini_3_flash(model_id) || is_gemma_4(model_id) {
        Some(GoogleThinkingLevel::Minimal)
    } else {
        None
    };
    ThinkingConfig {
        include_thoughts: None,
        thinking_budget: level.is_none().then_some(0.0),
        thinking_level: level,
    }
}

/// Level for a level-based family; pro and Gemma have no middle levels.
fn thinking_level(effort: ThinkingLevel, model_id: &str) -> GoogleThinkingLevel {
    use GoogleThinkingLevel::{High, Low, Medium, Minimal};
    let low_end = matches!(effort, ThinkingLevel::Minimal | ThinkingLevel::Low);
    if is_gemini_3_pro(model_id) {
        return if low_end { Low } else { High };
    }
    if is_gemma_4(model_id) {
        return if low_end { Minimal } else { High };
    }
    match effort {
        ThinkingLevel::Minimal => Minimal,
        ThinkingLevel::Low => Low,
        ThinkingLevel::Medium => Medium,
        ThinkingLevel::High | ThinkingLevel::Xhigh => High,
    }
}

/// Selected custom budget, else the family default; unknown families use dynamic (-1).
fn budget(model_id: &str, effort: ThinkingLevel, custom: Option<&ThinkingBudgets>) -> f64 {
    let (slot, index) = match effort {
        ThinkingLevel::Minimal => (custom.and_then(|c| c.minimal), 0),
        ThinkingLevel::Low => (custom.and_then(|c| c.low), 1),
        ThinkingLevel::Medium => (custom.and_then(|c| c.medium), 2),
        ThinkingLevel::High | ThinkingLevel::Xhigh => (custom.and_then(|c| c.high), 3),
    };
    let defaults = if model_id.contains("2.5-pro") {
        [128.0, 2048.0, 8192.0, 32768.0]
    } else if model_id.contains("2.5-flash-lite") {
        [512.0, 2048.0, 8192.0, 24576.0]
    } else if model_id.contains("2.5-flash") {
        [128.0, 2048.0, 8192.0, 24576.0]
    } else {
        [-1.0; 4]
    };
    slot.unwrap_or(defaults[index])
}
