//! The gateway's service addresses and the models it serves.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ModelCost, ModelInput, ProviderModelConfig};

/// Where sign-in and direct access tokens are issued.
pub const GITLAB_COM_URL: &str = "https://gitlab.com";
/// The gateway that serves model requests.
pub const AI_GATEWAY_URL: &str = "https://cloud.gitlab.com";

/// The built-in protocol that carries a model's requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// Messages protocol, served for Claude models.
    Anthropic,
    /// Responses protocol, served for GPT models.
    OpenAi,
}

impl Backend {
    /// The built-in protocol's identifier.
    #[must_use]
    pub const fn api(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic-messages",
            Self::OpenAi => "openai-responses",
        }
    }

    /// The gateway's proxy address for this protocol.
    #[must_use]
    pub fn base_url(self) -> String {
        let path = match self {
            Self::Anthropic => "/ai/v1/proxy/anthropic/",
            Self::OpenAi => "/ai/v1/proxy/openai/v1",
        };
        format!("{AI_GATEWAY_URL}{path}")
    }
}

/// One model the gateway serves; every model reasons and accepts text and images.
#[derive(Clone, Debug, PartialEq)]
pub struct GitLabModel {
    /// Exact model identifier.
    pub id: &'static str,
    /// Display label.
    pub name: &'static str,
    /// Protocol that carries the model's requests.
    pub backend: Backend,
    /// Prices per million tokens.
    pub cost: ModelCost,
    /// Context capacity in tokens.
    pub context_window: f64,
    /// Output capacity in tokens.
    pub max_tokens: f64,
}

/// Prices per million tokens: input, output, cache read and cache write.
const fn rates(input: f64, output: f64, cache_read: f64, cache_write: f64) -> ModelCost {
    ModelCost {
        input,
        output,
        cache_read,
        cache_write,
    }
}

/// The gateway's models, Claude through the messages protocol and GPT through responses.
pub static MODELS: [GitLabModel; 6] = [
    GitLabModel {
        id: "claude-opus-4-5-20251101",
        name: "Claude Opus 4.5",
        backend: Backend::Anthropic,
        cost: rates(15.0, 75.0, 1.5, 18.75),
        context_window: 200_000.0,
        max_tokens: 32_000.0,
    },
    GitLabModel {
        id: "claude-sonnet-4-5-20250929",
        name: "Claude Sonnet 4.5",
        backend: Backend::Anthropic,
        cost: rates(3.0, 15.0, 0.3, 3.75),
        context_window: 200_000.0,
        max_tokens: 16_384.0,
    },
    GitLabModel {
        id: "claude-haiku-4-5-20251001",
        name: "Claude Haiku 4.5",
        backend: Backend::Anthropic,
        cost: rates(1.0, 5.0, 0.1, 1.25),
        context_window: 200_000.0,
        max_tokens: 8_192.0,
    },
    GitLabModel {
        id: "gpt-5.1-2025-11-13",
        name: "GPT-5.1",
        backend: Backend::OpenAi,
        cost: rates(2.5, 10.0, 0.0, 0.0),
        context_window: 128_000.0,
        max_tokens: 16_384.0,
    },
    GitLabModel {
        id: "gpt-5-mini-2025-08-07",
        name: "GPT-5 Mini",
        backend: Backend::OpenAi,
        cost: rates(0.15, 0.6, 0.0, 0.0),
        context_window: 128_000.0,
        max_tokens: 16_384.0,
    },
    GitLabModel {
        id: "gpt-5-codex",
        name: "GPT-5 Codex",
        backend: Backend::OpenAi,
        cost: rates(2.5, 10.0, 0.0, 0.0),
        context_window: 128_000.0,
        max_tokens: 16_384.0,
    },
];

/// The table row for a model identifier.
#[must_use]
pub fn find(id: &str) -> Option<&'static GitLabModel> {
    MODELS.iter().find(|model| model.id == id)
}

/// The registration records for the table, without the protocol or address.
#[must_use]
pub fn provider_models() -> Vec<ProviderModelConfig> {
    MODELS
        .iter()
        .map(|model| ProviderModelConfig {
            id: model.id.into(),
            name: model.name.into(),
            api: None,
            base_url: None,
            reasoning: true,
            thinking_level_map: None,
            input: vec![ModelInput::Text, ModelInput::Image],
            cost: model.cost.clone(),
            context_window: model.context_window,
            max_tokens: model.max_tokens,
            headers: None,
            compat: None,
        })
        .collect()
}
