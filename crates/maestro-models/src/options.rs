//! Registered request capabilities and owned request choices.

use crate::{AuthResolver, Cancellation, Failure, RequestAuth};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Ordered request effort; default Off is not an application startup preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThinkingLevel {
    /// No enabled reasoning, unless an effort mapping explicitly declares otherwise.
    #[default]
    Off,
    /// Minimal reasoning.
    Minimal,
    /// Low reasoning.
    Low,
    /// Medium reasoning.
    Medium,
    /// High reasoning.
    High,
    /// Extra-high reasoning; requires explicit non-disabled support.
    Xhigh,
}

/// Declared reasoning conversion, independent of model or protocol names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThinkingMode {
    /// Map supported levels to effort strings (the default).
    #[default]
    Effort,
    /// Share the model output ceiling between output and reasoning tokens.
    TokenBudget,
}

/// Registered chat behavior. Default is nonreasoning with unknown output ceiling
/// and no optional preference support; names never imply capabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestCapabilities {
    /// Whether reasoning is supported at all.
    pub reasoning: bool,
    /// Positive shared output ceiling; nonpositive means unknown.
    pub output_limit: i64,
    /// Effort-string or token-budget conversion.
    pub thinking_mode: ThinkingMode,
    /// Missing entries use standard defaults; None disables a level; Some maps it.
    /// Off through High default to supported; Xhigh needs an explicit Some.
    pub thinking_level_map: BTreeMap<ThinkingLevel, Option<String>>,
    /// Whether a supplied temperature is accepted.
    pub temperature: bool,
    /// Supported supplied transport identifiers, treated as opaque data.
    pub transports: BTreeSet<String>,
    /// Supported supplied cache preference identifiers, treated as opaque data.
    pub cache_preferences: BTreeSet<String>,
    /// Whether supplied session affinity is accepted.
    pub session_affinity: bool,
}

/// Caller-composed request choices; defaults to Off, absent preferences and
/// empty headers and no authentication. Each default creates an independent
/// cancellation signal; supplied authentication and headers belong to dispatch.
#[derive(Clone, Default)]
pub struct StreamOptions {
    /// Supplied signal intentionally shared by request clones.
    pub cancellation: Cancellation,
    /// Requested reasoning choice.
    pub thinking: ThinkingLevel,
    /// Optional temperature; no default is synthesized.
    pub temperature: Option<f64>,
    /// Explicit output allowance, including zero; not capped by simple defaults.
    pub output_limit: Option<u64>,
    /// Supplied transport preference.
    pub transport: Option<String>,
    /// Supplied cache preference.
    pub cache_preference: Option<String>,
    /// Supplied session affinity.
    pub session_affinity: Option<String>,
    /// Explicit request authentication, taking precedence over resolution.
    pub auth: Option<RequestAuth>,
    /// Optional resolver for the selected provider only.
    pub auth_resolver: Option<Arc<dyn AuthResolver>>,
    /// Literal request header values; sensitive and not safe to log.
    pub headers: BTreeMap<String, String>,
}

/// Owned resolved request for adapters; clones share only cancellation's signal.
/// Effort and budget are mutually exclusive. Unsupported preferences are absent.
#[derive(Clone)]
pub struct EffectiveOptions {
    /// Original signal shared with caller options and stream normalization.
    pub cancellation: Cancellation,
    /// Original application-supplied reasoning choice.
    pub requested_thinking: ThinkingLevel,
    /// Supported choice after upward-first, then downward clamping.
    /// Nonreasoning or entirely disabled declarations fall back to Off.
    pub thinking: ThinkingLevel,
    /// Declared verbatim effort or lowercase default; unmapped Off has none.
    pub effort: Option<String>,
    /// Enabled token budget: 1024/2048/8192/16384; Xhigh aliases High.
    /// At output <= budget, reduced to output.saturating_sub(1024).
    pub thinking_budget: Option<u64>,
    /// Supported supplied temperature, without a reasoning overlay.
    pub temperature: Option<f64>,
    /// Explicit base allowance or min(positive ceiling, 32000), otherwise absent.
    /// Enabled budgets use min(base + budget, ceiling), without overflow.
    pub output_limit: Option<u64>,
    /// Supported supplied transport, otherwise absent.
    pub transport: Option<String>,
    /// Supported supplied cache preference, otherwise absent.
    pub cache_preference: Option<String>,
    /// Supported supplied session affinity, otherwise absent.
    pub session_affinity: Option<String>,
}

pub(crate) fn resolve(
    capabilities: &RequestCapabilities,
    options: StreamOptions,
) -> Result<EffectiveOptions, Failure> {
    let levels = [
        ThinkingLevel::Off,
        ThinkingLevel::Minimal,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Xhigh,
    ];
    let supported = |level: ThinkingLevel| {
        capabilities.reasoning
            && match capabilities.thinking_level_map.get(&level) {
                Some(value) => value.is_some(),
                None => level != ThinkingLevel::Xhigh,
            }
    };
    let thinking = levels
        .iter()
        .copied()
        .filter(|level| *level >= options.thinking)
        .find(|level| supported(*level))
        .or_else(|| levels.iter().rev().copied().find(|level| supported(*level)))
        .unwrap_or(ThinkingLevel::Off);
    let effort = if capabilities.reasoning && capabilities.thinking_mode == ThinkingMode::Effort {
        capabilities
            .thinking_level_map
            .get(&thinking)
            .cloned()
            .unwrap_or_else(|| match thinking {
                ThinkingLevel::Off => None,
                ThinkingLevel::Minimal => Some("minimal".into()),
                ThinkingLevel::Low => Some("low".into()),
                ThinkingLevel::Medium => Some("medium".into()),
                ThinkingLevel::High => Some("high".into()),
                ThinkingLevel::Xhigh => Some("xhigh".into()),
            })
    } else {
        None
    };
    let mut output_limit = options.output_limit.or_else(|| {
        (capabilities.output_limit > 0).then(|| (capabilities.output_limit as u64).min(32_000))
    });
    let thinking_budget = if capabilities.thinking_mode == ThinkingMode::TokenBudget
        && thinking != ThinkingLevel::Off
    {
        if capabilities.output_limit <= 0 {
            return Err(Failure::UnsupportedOperation);
        }
        let budget = match thinking {
            ThinkingLevel::Minimal => 1024,
            ThinkingLevel::Low => 2048,
            ThinkingLevel::Medium => 8192,
            ThinkingLevel::High | ThinkingLevel::Xhigh => 16384,
            ThinkingLevel::Off => unreachable!(),
        };
        output_limit = Some(
            output_limit
                .unwrap_or(0)
                .saturating_add(budget)
                .min(capabilities.output_limit as u64),
        );
        Some(if output_limit.unwrap() <= budget {
            output_limit.unwrap().saturating_sub(1024)
        } else {
            budget
        })
    } else {
        None
    };
    Ok(EffectiveOptions {
        cancellation: options.cancellation,
        requested_thinking: options.thinking,
        thinking,
        effort,
        thinking_budget,
        temperature: options.temperature.filter(|_| capabilities.temperature),
        output_limit,
        transport: options
            .transport
            .filter(|value| capabilities.transports.contains(value)),
        cache_preference: options
            .cache_preference
            .filter(|value| capabilities.cache_preferences.contains(value)),
        session_affinity: options
            .session_affinity
            .filter(|_| capabilities.session_affinity),
    })
}
