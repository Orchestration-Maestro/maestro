//! Provider configuration and binding records.
use super::ExtensionExecution;
use maestro_models::{Execution, SharedHandle, Threaded};
/// Native callable operations supplied with a provider registration.
pub use maestro_models_registry::ProviderCallbacks;
/// Configuration for registering a provider.
pub use maestro_models_registry::ProviderConfigInput as ProviderConfig;
/// Settings for one model inside a provider.
pub use maestro_models_registry::ProviderModelInput as ProviderModelConfig;
use std::sync::RwLock;

/// Registers a provider in place of the catalog.
pub type RegisterProviderHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::RegisterProvider>;
/// Unregisters a provider in place of the catalog.
pub type UnregisterProviderHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::UnregisterProvider>;

/// Provider operations bound in place of the runner's catalog; each may be absent.
#[derive(Clone, Default)]
pub struct ProviderActions<E: ExtensionExecution = Threaded> {
    /// Registers a provider; the catalog does it when absent.
    pub register_provider: Option<RegisterProviderHandler<E>>,
    /// Unregisters a provider; the catalog does it when absent.
    pub unregister_provider: Option<UnregisterProviderHandler<E>>,
}

/// A provider registration kept until a runner binds.
#[derive(Clone)]
pub struct PendingProviderRegistration {
    /// Provider name.
    pub name: String,
    /// The registered configuration: the platform handle, browser-local or natively shared.
    pub config: SharedHandle<RwLock<ProviderConfig>>,
    /// Path of the registering extension.
    pub extension_path: String,
}
