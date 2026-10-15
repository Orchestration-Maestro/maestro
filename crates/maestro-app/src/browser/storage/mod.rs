//! Injected browser collection storage.
pub mod store;
pub mod stores;
pub mod types;
pub use store::Store;
pub use stores::custom_providers_store::{
    AutoDiscoveryProviderType, CustomProvider, CustomProviderType, ManualProviderType,
};
pub use stores::{
    custom_providers_store::CustomProvidersStore, provider_keys_store::ProviderKeysStore,
    settings_store::SettingsStore,
};
pub use types::*;
