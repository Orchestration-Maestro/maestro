//! Retained application collections and browser-realm storage selection.
use super::{
    CustomProvidersStore, ProviderKeysStore, QuotaInfo, SessionsStore, SettingsStore,
    StorageBackend,
};
use std::{cell::RefCell, io, rc::Rc};
/// Supplied collection handles and the application's own storage adapter.
pub struct AppStorage {
    /// Application adapter; construction does not bind the collections.
    pub backend: Rc<dyn StorageBackend>,
    /// Supplied settings collection.
    pub settings: Rc<SettingsStore>,
    /// Supplied credential collection.
    pub provider_keys: Rc<ProviderKeysStore>,
    /// Supplied session collection.
    pub sessions: Rc<SessionsStore>,
    /// Supplied provider collection.
    pub custom_providers: Rc<CustomProvidersStore>,
}
impl AppStorage {
    /// Retain the exact supplied handles without rebinding them.
    #[must_use]
    pub fn new(
        settings: Rc<SettingsStore>,
        provider_keys: Rc<ProviderKeysStore>,
        sessions: Rc<SessionsStore>,
        custom_providers: Rc<CustomProvidersStore>,
        backend: Rc<dyn StorageBackend>,
    ) -> Self {
        Self {
            backend,
            settings,
            provider_keys,
            sessions,
            custom_providers,
        }
    }
    /// Inspect quota through the application's adapter.
    ///
    /// # Errors
    /// Returns the adapter's failure unchanged.
    pub async fn get_quota_info(&self) -> io::Result<QuotaInfo> {
        self.backend.get_quota_info().await
    }
    /// Request persistence through the application's adapter.
    ///
    /// # Errors
    /// Returns the adapter's failure unchanged.
    pub async fn request_persistence(&self) -> io::Result<bool> {
        self.backend.request_persistence().await
    }
}
thread_local! {
    /// Browser-realm-local retained application storage.
    static STORAGE: RefCell<Option<Rc<AppStorage>>> = const { RefCell::new(None) };
}
/// Retain the current application storage.
///
/// # Errors
/// Returns `NotConnected` before initialization.
pub fn get_app_storage() -> io::Result<Rc<AppStorage>> {
    STORAGE.with(|slot| slot.borrow().clone()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotConnected,
            "AppStorage not initialized. Call setAppStorage() first.",
        )
    })
}
/// Publish storage, releasing the previous handle outside the slot borrow.
pub fn set_app_storage(storage: Rc<AppStorage>) {
    let previous = STORAGE.with(|slot| slot.replace(Some(storage)));
    drop(previous);
}
