//! Which model a request is for, and the model-specific thinking and caching decisions.

#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};

#[cfg(not(target_arch = "wasm32"))]
use super::BedrockOptions;
#[cfg(not(target_arch = "wasm32"))]
use super::client::configured_region;
#[cfg(not(target_arch = "wasm32"))]
use super::proxy::Environment;
#[cfg(not(target_arch = "wasm32"))]
use super::wire::CachePointBlock;
use crate::Model;
use crate::arguments::json_parse::whitespace;
#[cfg(not(target_arch = "wasm32"))]
use crate::options::thinking_budget;
#[cfg(not(target_arch = "wasm32"))]
use crate::providers::messages::anthropic::{
    AnthropicThinkingDisplay, map_thinking_level_to_effort,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{CacheRetention, ThinkingLevel};

/// The lowercase identifier and nonempty name, each also with every run of whitespace,
/// underscores, dots and colons collapsed into one hyphen.
fn match_candidates(model: &Model) -> Vec<String> {
    let name = Some(model.name.as_str()).filter(|name| !name.is_empty());
    std::iter::once(model.id.as_str())
        .chain(name)
        .flat_map(|value| {
            let lower = value.to_lowercase();
            let mut hyphenated = String::with_capacity(lower.len());
            let mut in_run = false;
            for character in lower.chars() {
                let separator = whitespace(character) || matches!(character, '_' | '.' | ':');
                let normalized = if separator { '-' } else { character };
                if !(separator && in_run) {
                    hyphenated.push(normalized);
                }
                in_run = separator;
            }
            [lower, hyphenated]
        })
        .collect()
}

/// Whether a candidate of the model contains `needle`.
fn mentions(model: &Model, needle: &str) -> bool {
    match_candidates(model)
        .iter()
        .any(|candidate| candidate.contains(needle))
}

/// Whether the model is a Claude model, by identifier or by name.
pub(super) fn is_anthropic_claude_model(model: &Model) -> bool {
    let id = model.id.to_lowercase();
    id.contains("anthropic.claude")
        || id.contains("anthropic/claude")
        || model.name.to_lowercase().contains("claude")
}

/// Whether the model thinks adaptively (Opus 4.6 to 4.8, Sonnet 4.6 and Sonnet 5).
pub(super) fn supports_adaptive_thinking(model: &Model) -> bool {
    ["opus-4-6", "opus-4-7", "opus-4-8", "sonnet-4-6", "sonnet-5"]
        .iter()
        .any(|version| mentions(model, version))
}

/// Whether prompt caching is supported: candidates containing `claude` also need `-4-`,
/// `claude-3-7-sonnet` or `claude-3-5-haiku`; without `claude`, support instead requires
/// `AWS_BEDROCK_FORCE_CACHE` to be exactly `1`.
#[cfg(not(target_arch = "wasm32"))]
fn supports_prompt_caching(model: &Model, env: &Environment) -> bool {
    if !mentions(model, "claude") {
        return env.get("AWS_BEDROCK_FORCE_CACHE") == Some("1");
    }
    ["-4-", "claude-3-7-sonnet", "claude-3-5-haiku"]
        .iter()
        .any(|version| mentions(model, version))
}

/// The cache point to place after the system prompt and the final user turn, if any.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn cache_point(
    model: &Model,
    retention: CacheRetention,
    env: &Environment,
) -> Option<CachePointBlock> {
    (retention != CacheRetention::None && supports_prompt_caching(model, env)).then(|| {
        CachePointBlock {
            kind: Some("default".to_owned()),
            ttl: (retention == CacheRetention::Long).then(|| "1h".to_owned()),
        }
    })
}

/// Whether the request targets `GovCloud`, by region or by model identifier.
#[cfg(not(target_arch = "wasm32"))]
fn is_gov_cloud_target<E: crate::ProviderExecution>(
    model: &Model,
    options: &BedrockOptions<E>,
    env: &Environment,
) -> bool {
    let id = model.id.to_lowercase();
    configured_region(options, env)
        .is_some_and(|region| region.to_lowercase().starts_with("us-gov-"))
        || id.starts_with("us-gov.")
        || id.starts_with("arn:aws-us-gov:")
}

/// The thinking members of a Claude request, or none without reasoning, for a model that does
/// not reason and for any other model.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn additional_model_request_fields<E: crate::ProviderExecution>(
    model: &Model,
    options: &BedrockOptions<E>,
    env: &Environment,
) -> Option<Value> {
    let level = options.reasoning.filter(|_| model.reasoning)?;
    if !is_anthropic_claude_model(model) {
        return None;
    }
    let adaptive = supports_adaptive_thinking(model);
    let mut thinking = if adaptive {
        json!({"type": "adaptive"})
    } else {
        let budget = thinking_budget(level, options.thinking_budgets.as_ref());
        json!({"type": "enabled", "budget_tokens": budget})
    };
    if !is_gov_cloud_target(model, options, env) {
        let display = options
            .thinking_display
            .unwrap_or(AnthropicThinkingDisplay::Summarized);
        thinking["display"] = json!(display.as_str());
    }
    let mut fields = json!({"thinking": thinking});
    if adaptive {
        let native_xhigh = ["opus-4-7", "opus-4-8"]
            .iter()
            .any(|version| mentions(model, version));
        let effort = if level == ThinkingLevel::Xhigh && native_xhigh {
            "xhigh".to_owned()
        } else {
            map_thinking_level_to_effort(model, level)
        };
        fields["output_config"] = json!({"effort": effort});
    } else if options.interleaved_thinking.unwrap_or(true) {
        fields["anthropic_beta"] = json!(["interleaved-thinking-2025-05-14"]);
    }
    Some(fields)
}
