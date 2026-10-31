//! Ordered thinking metadata recognition.
use crate::{Model, ModelThinkingLevel as Level};

/// Merge only the explicitly supplied levels.
pub(super) fn merge_thinking_level_map(model: &mut Model, entries: &[(Level, Option<&str>)]) {
    model.thinking_level_map.get_or_insert_default().extend(
        entries
            .iter()
            .map(|(level, value)| (level.clone(), value.map(str::to_owned))),
    );
}
/// Direct Responses models whose reasoning effort accepts `none`.
const OPENAI_RESPONSES_NONE_REASONING_MODELS: &[&str] = &[
    "gpt-5.1",
    "gpt-5.2",
    "gpt-5.3-codex",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "gpt-5.5",
];
/// Recognize response models supporting the extra-high level.
fn supports_open_ai_xhigh(id: &str) -> bool {
    ["gpt-5.2", "gpt-5.3", "gpt-5.4", "gpt-5.5"]
        .iter()
        .any(|part| id.contains(part))
}
/// Recognize an unanchored Gemini version with an optional ASCII numeric suffix.
fn is_gemini3_model(id: &str, family: &str) -> bool {
    id.match_indices("gemini-3").any(|(offset, _)| {
        let mut suffix = &id[offset + "gemini-3".len()..];
        if let Some(version) = suffix.strip_prefix('.') {
            let digits = version.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                return false;
            }
            suffix = &version[digits..];
        }
        suffix.starts_with(family)
    })
}
/// Add the completion protocol's `DeepSeek` effort mappings.
pub(super) fn deepseek_levels(model: &mut Model) {
    let xhigh = if model.provider == "openrouter" {
        "xhigh"
    } else {
        "max"
    };
    merge_thinking_level_map(
        model,
        &[
            (Level::Minimal, None),
            (Level::Low, None),
            (Level::Medium, None),
            (Level::High, Some("high")),
            (Level::Xhigh, Some(xhigh)),
        ],
    );
}
/// Apply the `OpenAI` model-family rules in order.
fn openai_levels(model: &mut Model) {
    if matches!(
        model.api.as_str(),
        "openai-responses" | "azure-openai-responses"
    ) && model.id.starts_with("gpt-5")
    {
        merge_thinking_level_map(model, &[(Level::Off, None)]);
    }
    if model.api == "openai-responses"
        && model.provider == "openai"
        && OPENAI_RESPONSES_NONE_REASONING_MODELS.contains(&model.id.as_str())
    {
        merge_thinking_level_map(model, &[(Level::Off, Some("none"))]);
    }
    if supports_open_ai_xhigh(&model.id) {
        merge_thinking_level_map(model, &[(Level::Xhigh, Some("xhigh"))]);
    }
    if model.provider == "openai" && model.id == "gpt-5.5" {
        merge_thinking_level_map(model, &[(Level::Minimal, None)]);
    }
    if model.id.ends_with("gpt-5.5-pro") {
        merge_thinking_level_map(
            model,
            &[
                (Level::Off, None),
                (Level::Minimal, None),
                (Level::Low, None),
            ],
        );
    }
}
/// Apply the provider-specific rules that follow the shared families.
fn provider_levels(model: &mut Model) {
    if model.provider == "groq" && model.id == "qwen/qwen3-32b" {
        merge_thinking_level_map(
            model,
            &[
                (Level::Minimal, None),
                (Level::Low, None),
                (Level::Medium, None),
                (Level::High, Some("default")),
            ],
        );
    }
    if model.provider == "opencode-go" && model.id == "kimi-k2.6" {
        merge_thinking_level_map(
            model,
            &[
                (Level::Minimal, None),
                (Level::Low, None),
                (Level::Medium, None),
            ],
        );
    }
    if model.provider == "openai-codex" && supports_open_ai_xhigh(&model.id) {
        merge_thinking_level_map(model, &[(Level::Minimal, Some("low"))]);
    }
    if model.provider == "openai-codex" && model.id == "gpt-5.1-codex-mini" {
        merge_thinking_level_map(
            model,
            &[
                (Level::Minimal, Some("medium")),
                (Level::Low, Some("medium")),
                (Level::Medium, Some("medium")),
                (Level::High, Some("high")),
            ],
        );
    }
}
/// Apply metadata in rule order independently of reasoning capability.
pub(super) fn apply_thinking_level_metadata(model: &mut Model) {
    openai_levels(model);
    if ["opus-4-6", "opus-4.6"]
        .iter()
        .any(|part| model.id.contains(part))
    {
        merge_thinking_level_map(model, &[(Level::Xhigh, Some("max"))]);
    }
    if ["opus-4-7", "opus-4.7"]
        .iter()
        .any(|part| model.id.contains(part))
    {
        merge_thinking_level_map(model, &[(Level::Xhigh, Some("xhigh"))]);
    }
    if model.api == "openai-completions" && model.id.contains("deepseek-v4") {
        deepseek_levels(model);
    }
    if matches!(model.api.as_str(), "google-generative-ai" | "google-vertex") {
        google_levels(model);
    }
    provider_levels(model);
}
/// Apply the three case-insensitive Google-family rules in order.
fn google_levels(model: &mut Model) {
    let id = model.id.to_lowercase();
    if is_gemini3_model(&id, "-pro") {
        merge_thinking_level_map(
            model,
            &[
                (Level::Off, None),
                (Level::Minimal, None),
                (Level::Low, Some("LOW")),
                (Level::Medium, None),
                (Level::High, Some("HIGH")),
            ],
        );
    }
    if is_gemini3_model(&id, "-flash") {
        merge_thinking_level_map(model, &[(Level::Off, None)]);
    }
    if id.contains("gemma4") || id.contains("gemma-4") {
        merge_thinking_level_map(
            model,
            &[
                (Level::Off, None),
                (Level::Minimal, Some("MINIMAL")),
                (Level::Low, None),
                (Level::Medium, None),
                (Level::High, Some("HIGH")),
            ],
        );
    }
}
