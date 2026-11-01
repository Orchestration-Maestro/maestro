//! Local-server model discovery over the page's own fetch and socket effects.
use maestro_app::{
    AutoDiscoveryProviderType, Model,
    browser::utils::model_discovery::{
        DiscoveryDiagnostic, DiscoveryError, DiscoveryOptions, DiscoveryProvider, discover_models,
    },
    default_fetch,
};
use std::sync::Arc;

/// List the models a local server of `kind` offers, logging its diagnostics to the console.
/// # Errors
/// Returns the server's discovery failure.
pub(crate) async fn discover(
    kind: AutoDiscoveryProviderType,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<Model>, DiscoveryError> {
    let provider = match kind {
        AutoDiscoveryProviderType::Ollama => DiscoveryProvider::Ollama,
        AutoDiscoveryProviderType::LlamaCpp => DiscoveryProvider::LlamaCpp,
        AutoDiscoveryProviderType::Vllm => DiscoveryProvider::Vllm,
        AutoDiscoveryProviderType::Lmstudio => DiscoveryProvider::Lmstudio,
    };
    let options = DiscoveryOptions {
        fetch: default_fetch(),
        log: Arc::new(|diagnostic: DiscoveryDiagnostic<'_>| {
            let text = diagnostic.to_string().into();
            match diagnostic {
                DiscoveryDiagnostic::SkippedWithoutTools { .. } => web_sys::console::debug_1(&text),
                _ => web_sys::console::error_1(&text),
            }
        }),
        connect_web_socket: super::websocket::browser_web_socket(),
    };
    discover_models(provider, base_url, api_key, &options).await
}
