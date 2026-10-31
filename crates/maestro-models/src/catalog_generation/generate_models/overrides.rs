//! Ordered corrections to acquired descriptors.
use crate::{Model, ModelCompat};

/// Copilot models offering the extended one-million-token context window.
const GITHUB_COPILOT_EXTENDED_CONTEXT_MODELS: &[&str] = &[
    "claude-fable-5",
    "claude-opus-4.6",
    "claude-opus-4.7",
    "claude-opus-4.8",
    "claude-sonnet-4.6",
    "claude-sonnet-5",
    "gpt-5.3-codex",
    "gpt-5.4",
    "gpt-5.5",
];
/// Repair acquired cache prices, contexts and router rates in place.
pub(super) fn correct_models(models: &mut [Model]) {
    if let Some(opus) = models
        .iter_mut()
        .find(|model| model.provider == "anthropic" && model.id == "claude-opus-4-5")
    {
        opus.cost.cache_read = 0.5;
        opus.cost.cache_write = 6.25;
    }
    for model in models {
        if model.provider == "amazon-bedrock" && model.id.contains("anthropic.claude-opus-4-6-v1") {
            model.cost.cache_read = 0.5;
            model.cost.cache_write = 6.25;
        }
        if model.provider == "github-copilot"
            && GITHUB_COPILOT_EXTENDED_CONTEXT_MODELS.contains(&model.id.as_str())
        {
            model.context_window = 1_000_000.0;
        }
        if matches!(
            model.provider.as_str(),
            "anthropic" | "opencode" | "opencode-go"
        ) && matches!(
            model.id.as_str(),
            "claude-opus-4-6" | "claude-sonnet-4-6" | "claude-opus-4.6" | "claude-sonnet-4.6"
        ) {
            model.context_window = 1_000_000.0;
        }
        let opencode = matches!(model.provider.as_str(), "opencode" | "opencode-go");
        if opencode && matches!(model.id.as_str(), "claude-sonnet-4-5" | "claude-sonnet-4") {
            model.context_window = 200_000.0;
        }
        if (opencode && model.id == "gpt-5.4")
            || (model.provider == "openai" && matches!(model.id.as_str(), "gpt-5.4" | "gpt-5.5"))
        {
            model.context_window = 272_000.0;
            model.max_tokens = 128_000.0;
        }
        if model.provider == "openrouter" {
            router_prices(model);
        }
        if model.provider == "openrouter" && model.id.starts_with("moonshotai/kimi-k2.6") {
            let compat = model.compat.get_or_insert_with(ModelCompat::default);
            compat
                .0
                .insert("supportsDeveloperRole".into(), false.into());
            compat.0.insert(
                "requiresReasoningContentOnAssistantMessages".into(),
                true.into(),
            );
        }
    }
}
/// Change only the authored routing price and limit fields.
fn router_prices(model: &mut Model) {
    match model.id.as_str() {
        "moonshotai/kimi-k2.5" => {
            model.cost.input = 0.41;
            model.cost.output = 2.06;
            model.cost.cache_read = 0.07;
            model.max_tokens = 4096.0;
        }
        "z-ai/glm-5" => {
            model.cost.input = 0.6;
            model.cost.output = 1.9;
            model.cost.cache_read = 0.119;
        }
        _ => {}
    }
}
/// Merge `DeepSeek` completion compatibility without discarding other fields.
pub(super) fn deepseek_compat(models: &mut [Model]) {
    for model in models
        .iter_mut()
        .filter(|model| model.api == "openai-completions" && model.id.contains("deepseek-v4"))
    {
        let router = model.provider == "openrouter";
        let compat = model.compat.get_or_insert_with(ModelCompat::default);
        compat.0.insert(
            "requiresReasoningContentOnAssistantMessages".into(),
            true.into(),
        );
        if !router {
            compat.0.insert("thinkingFormat".into(), "deepseek".into());
        }
        super::thinking::deepseek_levels(model);
    }
}
/// Retain only supported direct `MiniMax` models, keeping their upstream limits.
pub(super) fn minimax_models(models: &mut Vec<Model>) {
    models.retain_mut(|model| {
        if !matches!(model.provider.as_str(), "minimax" | "minimax-cn") {
            return true;
        }
        matches!(model.id.as_str(), "MiniMax-M2.7" | "MiniMax-M2.7-highspeed")
    });
}
