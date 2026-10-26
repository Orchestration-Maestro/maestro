//! Provider registration records an extension supplies.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

pub use maestro_request::provider_config::ProviderModelConfig;

/// Provider registration data; absent fields keep the host's earlier values.
pub type ProviderConfig = maestro_request::provider_config::ProviderConfig<ProviderCallbacks>;

/// Executable provider capabilities retained beside ordinary registration data.
#[derive(Clone, Default)]
pub struct ProviderCallbacks {
    /// Stream setup receives the supplied raw or simple options unchanged.
    pub stream_simple: Option<crate::StreamFunction<crate::StreamOptionsInput>>,
}
