#![doc = include_str!("../../../../docs/model-options.md")]
use crate::{Model, SimpleStreamOptions, StreamOptions, ThinkingBudgets, ThinkingLevel};

/// Copy common settings without invoking retained hooks.
///
/// Missing token limits use the positive model limit capped at 32,000.
/// A separately supplied nonempty key overrides the copied key without trimming.
#[must_use]
pub fn build_base_options(
    model: &Model,
    options: Option<&SimpleStreamOptions>,
    api_key: Option<&str>,
) -> StreamOptions {
    let mut common = options.map_or_else(StreamOptions::default, |options| options.common.clone());
    if common.max_tokens.is_none() && model.max_tokens > 0.0 {
        common.max_tokens = Some(model.max_tokens.min(32_000.0));
    }
    if let Some(key) = api_key.filter(|key| !key.is_empty()) {
        common.api_key = Some(key.to_owned());
    }
    common
}

/// Map extra-high reasoning to high; retain every other requested level.
#[must_use]
pub fn clamp_reasoning(effort: Option<ThinkingLevel>) -> Option<ThinkingLevel> {
    match effort {
        Some(ThinkingLevel::Xhigh) => Some(ThinkingLevel::High),
        other => other,
    }
}

/// Combined token limit and the thinking budget it can accommodate.
pub struct AdjustedMaxTokens {
    /// Maximum combined output and thinking tokens.
    pub max_tokens: f64,
    /// Thinking tokens, reduced only when the combined limit cannot fit them.
    pub thinking_budget: f64,
}

/// Add a per-level thinking budget, capped by the model's token limit.
///
/// Defaults are 1,024, 2,048, 8,192 and 16,384 for minimal through high.
/// Extra-high uses the high slot. Missing custom slots retain their defaults.
/// When the capped limit is no larger than the budget, reserve 1,024 output
/// tokens by reducing the budget, never below zero.
#[must_use]
pub fn adjust_max_tokens_for_thinking(
    base_max_tokens: f64,
    model_max_tokens: f64,
    reasoning_level: ThinkingLevel,
    custom_budgets: Option<&ThinkingBudgets>,
) -> AdjustedMaxTokens {
    let (custom, default) = match reasoning_level {
        ThinkingLevel::Minimal => (custom_budgets.and_then(|b| b.minimal), 1024.0),
        ThinkingLevel::Low => (custom_budgets.and_then(|b| b.low), 2048.0),
        ThinkingLevel::Medium => (custom_budgets.and_then(|b| b.medium), 8192.0),
        ThinkingLevel::High | ThinkingLevel::Xhigh => {
            (custom_budgets.and_then(|b| b.high), 16384.0)
        }
    };
    let budget = custom.unwrap_or(default);
    let max_tokens = (base_max_tokens + budget).min(model_max_tokens);
    let thinking_budget = if max_tokens <= budget {
        (max_tokens - 1024.0).max(0.0)
    } else {
        budget
    };
    AdjustedMaxTokens {
        max_tokens,
        thinking_budget,
    }
}
