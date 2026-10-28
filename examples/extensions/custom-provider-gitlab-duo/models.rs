//! The gateway's service addresses and the models it serves.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::ProviderModelConfig;

/// The built-in protocol that carries a model's requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// Messages protocol.
    Anthropic,
    /// Responses protocol.
    OpenAi,
}

impl Backend {
    /// The built-in protocol's identifier.
    #[must_use]
    pub const fn api(self) -> &'static str {
        match self {
            Self::Anthropic | Self::OpenAi => "",
        }
    }

    /// The gateway's proxy address for this protocol.
    #[must_use]
    pub fn base_url(self) -> String {
        match self {
            Self::Anthropic | Self::OpenAi => String::new(),
        }
    }
}

/// One model the gateway serves.
#[derive(Clone, Debug, PartialEq)]
pub struct GitLabModel {
    /// Protocol that carries the model's requests.
    pub backend: Backend,
}

/// The table, one row per protocol.
static MODELS: [GitLabModel; 2] = [
    GitLabModel {
        backend: Backend::Anthropic,
    },
    GitLabModel {
        backend: Backend::OpenAi,
    },
];

/// The table row for a model identifier.
#[must_use]
pub fn find(_id: &str) -> Option<&'static GitLabModel> {
    MODELS.first().filter(|_| false)
}

/// The registration records for the table.
#[must_use]
pub fn provider_models() -> Vec<ProviderModelConfig> {
    Vec::new()
}
