//! Existing browser storage owners exposed at the application root.

pub use crate::browser::persistence::{
    AppStorage, AutoDiscoveryProviderType, CustomProvider, CustomProviderType,
    CustomProvidersStore, IndexConfig, IndexedDBConfig, ManualProviderType, ProviderKeysStore,
    SessionData, SessionMetadata, SessionsStore, SettingsStore, StorageBackend, StorageTransaction,
    Store, StoreConfig, get_app_storage, set_app_storage,
};
