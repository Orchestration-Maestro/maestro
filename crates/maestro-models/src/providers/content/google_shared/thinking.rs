//! Shared thinking selection with independent endpoint family policies.
use super::super::generative::GoogleThinking;
use super::{GoogleThinkingLevel, ThinkingConfig, is_gemini_3_flash, is_gemini_3_pro, is_gemma_4};
use crate::{
    Model, ModelThinkingLevel, ThinkingBudgets, ThinkingLevel, clamp_reasoning,
    clamp_thinking_level,
};

/// Clamp a requested level to the model's support; off and extra-high select high.
pub(in crate::providers::content) fn effort(
    model: &Model,
    requested: ThinkingLevel,
) -> ThinkingLevel {
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
pub(in crate::providers::content) fn enabled_for_effort(
    model: &Model,
    effort: ThinkingLevel,
    budgets: Option<&ThinkingBudgets>,
    cloud: bool,
) -> GoogleThinking {
    let level_based = is_gemini_3_pro(&model.id)
        || is_gemini_3_flash(&model.id)
        || (!cloud && is_gemma_4(&model.id));
    if level_based {
        GoogleThinking {
            enabled: true,
            budget_tokens: None,
            level: Some(thinking_level(effort, &model.id)),
        }
    } else {
        GoogleThinking {
            enabled: true,
            budget_tokens: Some(budget(&model.id, effort, budgets, cloud)),
            level: None,
        }
    }
}

/// Lowest thinking setting a family accepts, without returning thoughts.
pub(in crate::providers::content) fn disabled_thinking(
    model_id: &str,
    cloud: bool,
) -> ThinkingConfig {
    let level = if is_gemini_3_pro(model_id) {
        Some(GoogleThinkingLevel::Low)
    } else if is_gemini_3_flash(model_id) || (!cloud && is_gemma_4(model_id)) {
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
fn budget(
    model_id: &str,
    effort: ThinkingLevel,
    custom: Option<&ThinkingBudgets>,
    cloud: bool,
) -> f64 {
    let (slot, index) = match effort {
        ThinkingLevel::Minimal => (custom.and_then(|c| c.minimal), 0),
        ThinkingLevel::Low => (custom.and_then(|c| c.low), 1),
        ThinkingLevel::Medium => (custom.and_then(|c| c.medium), 2),
        ThinkingLevel::High | ThinkingLevel::Xhigh => (custom.and_then(|c| c.high), 3),
    };
    let defaults = if model_id.contains("2.5-pro") {
        [128.0, 2048.0, 8192.0, 32768.0]
    } else if !cloud && model_id.contains("2.5-flash-lite") {
        [512.0, 2048.0, 8192.0, 24576.0]
    } else if model_id.contains("2.5-flash") {
        [128.0, 2048.0, 8192.0, 24576.0]
    } else {
        [-1.0; 4]
    };
    slot.unwrap_or(defaults[index])
}
