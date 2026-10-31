#![doc = include_str!("../../../../docs/model-options.md")]
use std::borrow::Cow;

use crate::{
    CacheRetention, Context, Model, ModelThinkingLevel, ProviderExecution, SimpleStreamOptions,
    StreamOptions, ThinkingBudgets, ThinkingLevel, get_env_api_key,
};

/// Keep a nonempty supplied key as written, otherwise look the provider's key up in the environment.
pub(crate) fn resolve_api_key<'a>(
    provider: &str,
    explicit: Option<Cow<'a, str>>,
) -> Option<Cow<'a, str>> {
    explicit
        .filter(|key| !key.is_empty())
        .or_else(|| get_env_api_key(provider).map(Cow::Owned))
}

/// Copy common settings without invoking retained hooks.
///
/// Missing token limits use the positive model limit capped at 32,000.
/// A separately supplied nonempty key overrides the copied key without trimming.
#[must_use]
pub fn build_base_options(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
    api_key: Option<&str>,
) -> StreamOptions {
    build_base_options_for(model, context, options, api_key)
}

/// Copy family-correct common options without invoking or transferring hooks.
#[must_use]
pub fn build_base_options_for<E: ProviderExecution>(
    model: &Model,
    _context: &Context,
    options: Option<&SimpleStreamOptions<E>>,
    api_key: Option<&str>,
) -> StreamOptions<E> {
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

/// The token base thinking budgets are added to: absent, zero and NaN limits count as zero.
pub(crate) fn token_base(max_tokens: Option<f64>) -> f64 {
    max_tokens
        .filter(|n| *n != 0.0 && !n.is_nan())
        .unwrap_or(0.0)
}

/// The budget of a level: the caller's slot when set (zero included), otherwise 1,024, 2,048,
/// 8,192 or 16,384 for minimal through high. Extra-high uses the high slot.
pub(crate) fn thinking_budget(level: ThinkingLevel, custom: Option<&ThinkingBudgets>) -> f64 {
    let (slot, default) = match level {
        ThinkingLevel::Minimal => (custom.and_then(|b| b.minimal), 1024.0),
        ThinkingLevel::Low => (custom.and_then(|b| b.low), 2048.0),
        ThinkingLevel::Medium => (custom.and_then(|b| b.medium), 8192.0),
        ThinkingLevel::High | ThinkingLevel::Xhigh => (custom.and_then(|b| b.high), 16384.0),
    };
    slot.unwrap_or(default)
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
/// tokens by reducing the budget, never below zero. A NaN sum or model limit
/// produces a NaN combined limit.
#[must_use]
pub fn adjust_max_tokens_for_thinking(
    base_max_tokens: Option<f64>,
    model_max_tokens: f64,
    reasoning_level: ThinkingLevel,
    custom_budgets: Option<&ThinkingBudgets>,
) -> AdjustedMaxTokens {
    let budget = thinking_budget(reasoning_level, custom_budgets);
    let combined = base_max_tokens.unwrap_or(0.0) + budget;
    let max_tokens = if combined.is_nan() || model_max_tokens.is_nan() {
        f64::NAN
    } else {
        combined.min(model_max_tokens)
    };
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

/// Convert an already-selected thinking level without choosing a model or clamping support.
#[must_use]
pub fn reasoning_from_thinking_level(level: &ModelThinkingLevel) -> Option<ThinkingLevel> {
    match level {
        ModelThinkingLevel::Off => None,
        ModelThinkingLevel::Minimal => Some(ThinkingLevel::Minimal),
        ModelThinkingLevel::Low => Some(ThinkingLevel::Low),
        ModelThinkingLevel::Medium => Some(ThinkingLevel::Medium),
        ModelThinkingLevel::High => Some(ThinkingLevel::High),
        ModelThinkingLevel::Xhigh => Some(ThinkingLevel::Xhigh),
    }
}

/// Environment variable that selects long cache retention when set to `long`.
const CACHE_RETENTION_VARIABLE: &str = "MAESTRO_CACHE_RETENTION";

/// Resolve an explicit retention choice before the environment default.
pub(crate) fn resolve_cache_retention(explicit: Option<CacheRetention>) -> CacheRetention {
    match explicit {
        Some(retention) => retention,
        None if std::env::var(CACHE_RETENTION_VARIABLE).is_ok_and(|value| value == "long") => {
            CacheRetention::Long
        }
        None => CacheRetention::Short,
    }
}
