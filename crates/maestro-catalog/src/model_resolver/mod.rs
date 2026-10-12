//! Model references resolved against shared catalog descriptors.
mod matching;
pub use matching::{find_exact_model_reference_match, parse_model_pattern};
mod collation;
pub use collation::model_id_comparator;
use maestro_models::{Model, ModelThinkingLevel};
use std::sync::{Arc, RwLock};
/// Model selection with independently optional thinking and warning.
#[derive(Default)]
pub struct ParsedModelResult {
    /// Selected shared descriptor, if any.
    pub model: Option<Arc<RwLock<Model>>>,
    /// Explicit suffix effort, without capability clamping.
    pub thinking_level: Option<ModelThinkingLevel>,
    /// Invalid-suffix diagnostic after successful fallback.
    pub warning: Option<String>,
}
/// Invalid thinking suffix policy.
#[derive(Clone, Copy)]
pub struct ParseModelPatternOptions {
    /// Resolve an invalid suffix through its prefix instead of rejecting it.
    pub allow_invalid_thinking_level_fallback: bool,
}
impl Default for ParseModelPatternOptions {
    fn default() -> Self {
        Self {
            allow_invalid_thinking_level_fallback: true,
        }
    }
}
mod cli;
use crate::{CatalogError, ModelRegistry};
pub use cli::resolve_cli_model;
/// Command-line operands with the caller's catalog.
#[derive(Clone, Copy)]
pub struct ResolveCliModelOptions<'a> {
    /// Explicit provider constraint.
    pub cli_provider: Option<&'a str>,
    /// Model reference or pattern.
    pub cli_model: Option<&'a str>,
    /// Catalog including models without configured authentication.
    pub model_registry: &'a ModelRegistry,
}
/// Command-line selection or catalog-owned diagnostic.
#[derive(Default)]
pub struct ResolveCliModelResult {
    /// Selected shared or detached custom descriptor.
    pub model: Option<Arc<RwLock<Model>>>,
    /// Explicit suffix effort.
    pub thinking_level: Option<ModelThinkingLevel>,
    /// Custom-ID fallback diagnostic.
    pub warning: Option<String>,
    /// Selection failure without process termination.
    pub error: Option<CatalogError>,
}

use indexmap::IndexMap;
use std::sync::LazyLock;
/// Ordered, extensible provider defaults used for detached custom model metadata.
pub static DEFAULT_MODEL_PER_PROVIDER: LazyLock<RwLock<IndexMap<String, String>>> =
    LazyLock::new(|| {
        RwLock::new(
            [
                ("amazon-bedrock", "us.anthropic.claude-opus-4-6-v1"),
                ("anthropic", "claude-opus-4-7"),
                ("openai", "gpt-5.4"),
                ("azure-openai-responses", "gpt-5.4"),
                ("openai-codex", "gpt-5.5"),
                ("deepseek", "deepseek-v4-pro"),
                ("google", "gemini-3.1-pro-preview"),
                ("google-vertex", "gemini-3.1-pro-preview"),
                ("github-copilot", "gpt-5.4"),
                ("openrouter", "moonshotai/kimi-k2.6"),
                ("vercel-ai-gateway", "zai/glm-5.1"),
                ("xai", "grok-4.20-0309-reasoning"),
                ("groq", "openai/gpt-oss-120b"),
                ("cerebras", "zai-glm-4.7"),
                ("zai", "glm-5.1"),
                ("mistral", "devstral-medium-latest"),
                ("minimax", "MiniMax-M2.7"),
                ("minimax-cn", "MiniMax-M2.7"),
                ("moonshotai", "kimi-k2.6"),
                ("moonshotai-cn", "kimi-k2.6"),
                ("huggingface", "moonshotai/Kimi-K2.6"),
                ("fireworks", "accounts/fireworks/models/kimi-k2p6"),
                ("opencode", "kimi-k2.6"),
                ("opencode-go", "kimi-k2.6"),
                ("kimi-coding", "kimi-for-coding"),
                ("cloudflare-workers-ai", "@cf/moonshotai/kimi-k2.6"),
                (
                    "cloudflare-ai-gateway",
                    "workers-ai/@cf/moonshotai/kimi-k2.6",
                ),
                ("xiaomi", "mimo-v2.5-pro"),
                ("xiaomi-token-plan-cn", "mimo-v2.5-pro"),
                ("xiaomi-token-plan-ams", "mimo-v2.5-pro"),
                ("xiaomi-token-plan-sgp", "mimo-v2.5-pro"),
            ]
            .into_iter()
            .map(|(provider, id)| (provider.into(), id.into()))
            .collect(),
        )
    });
