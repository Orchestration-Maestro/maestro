use crate::{Model, SimpleStreamOptions, StreamOptions, ThinkingBudgets, ThinkingLevel};

/// Move supplied common options into a base request.
pub fn build_base_options(
    model: &Model,
    options: Option<SimpleStreamOptions>,
    api_key: Option<&str>,
) -> StreamOptions {
    let mut base = options.unwrap_or_default().base;
    if base.max_tokens.is_none() && model.max_tokens > 0.0 {
        base.max_tokens = Some(model.max_tokens.min(32000.0));
    }
    if let Some(key) = api_key.filter(|key| !key.is_empty()) {
        base.api_key = Some(key.to_owned());
    }
    base
}

/// Map the highest reasoning effort to the high token budget.
pub fn clamp_reasoning(effort: Option<ThinkingLevel>) -> Option<ThinkingLevel> {
    match effort {
        Some(ThinkingLevel::Xhigh) => Some(ThinkingLevel::High),
        other => other,
    }
}

/// Combined request limit and selected thinking allowance.
pub struct AdjustedMaxTokens {
    /// Combined token limit.
    pub max_tokens: f64,
    /// Thinking token allowance.
    pub thinking_budget: f64,
}

/// Select an individually overridden thinking budget and cap combined tokens.
pub fn adjust_max_tokens_for_thinking(
    base_max_tokens: f64,
    model_max_tokens: f64,
    reasoning_level: ThinkingLevel,
    custom_budgets: Option<&ThinkingBudgets>,
) -> AdjustedMaxTokens {
    let (supplied, default) = match clamp_reasoning(Some(reasoning_level)).unwrap() {
        ThinkingLevel::Minimal => (custom_budgets.and_then(|b| b.minimal), 1024.0),
        ThinkingLevel::Low => (custom_budgets.and_then(|b| b.low), 2048.0),
        ThinkingLevel::Medium => (custom_budgets.and_then(|b| b.medium), 8192.0),
        ThinkingLevel::High | ThinkingLevel::Xhigh => {
            (custom_budgets.and_then(|b| b.high), 16384.0)
        }
    };
    let mut thinking_budget = supplied.unwrap_or(default);
    let max_tokens = math_min(base_max_tokens + thinking_budget, model_max_tokens);
    if max_tokens <= thinking_budget {
        thinking_budget = math_max(0.0, max_tokens - 1024.0);
    }
    AdjustedMaxTokens {
        max_tokens,
        thinking_budget,
    }
}

// ECMAScript extrema propagate NaN and distinguish signed-zero ties.
fn math_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() || b.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else if a < b {
        a
    } else {
        b
    }
}
fn math_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_positive() || b.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else if a > b {
        a
    } else {
        b
    }
}
