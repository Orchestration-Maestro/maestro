//! Shared schema ownership for browser collections.
use super::types::{StorageBackend, StoreConfig};
use serde::{Serialize, de::DeserializeOwned};
use std::{cell::RefCell, io, rc::Rc};
/// Schema shared by a named collection's operations.
pub struct Store {
    /// Owned collection schema.
    config: StoreConfig,
    /// Replaceable adapter shared by future operations.
    backend: RefCell<Option<Rc<dyn StorageBackend>>>,
}
impl Store {
    /// Own a collection schema.
    #[must_use]
    pub fn new(config: StoreConfig) -> Self {
        Self {
            config,
            backend: RefCell::default(),
        }
    }
    /// Return an independent schema snapshot.
    pub fn get_config(&self) -> StoreConfig {
        self.config.clone()
    }

    /// Replace the adapter used by subsequent operations.
    pub fn set_backend(&self, backend: Rc<dyn StorageBackend>) {
        let previous = self.backend.replace(Some(backend));
        drop(previous);
    }
    /// Retain the currently selected adapter or report an unbound collection.
    ///
    /// # Errors
    /// Returns `NotConnected` if no backend has been bound.
    pub fn get_backend(&self) -> io::Result<Rc<dyn StorageBackend>> {
        self.backend
            .borrow()
            .clone()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Storage backend not set"))
    }
    /// Read and decode a value after selecting the adapter.
    pub(super) async fn read<T: DeserializeOwned>(&self, key: &str) -> io::Result<Option<T>> {
        self.read_from(&self.config.name, key).await
    }
    /// Read a named collection with the same absent/null decoding policy.
    pub(super) async fn read_from<T: DeserializeOwned>(
        &self,
        name: &str,
        key: &str,
    ) -> io::Result<Option<T>> {
        let backend = self.get_backend()?;
        backend
            .get(name, key)
            .await?
            .filter(|value| !value.is_null())
            .map(Self::decode)
            .transpose()
    }
    /// Decode a stored value without replacing its serde cause.
    pub(super) fn decode<T: DeserializeOwned>(value: serde_json::Value) -> io::Result<T> {
        serde_json::from_value(value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
    /// Encode at the consuming write boundary.
    pub(super) fn encode<T: Serialize + ?Sized>(value: &T) -> io::Result<serde_json::Value> {
        serde_json::to_value(value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
    /// Encode a value after selecting the adapter, then write it.
    pub(super) async fn write<T: Serialize + ?Sized>(
        &self,
        key: &str,
        value: &T,
    ) -> io::Result<()> {
        let backend = self.get_backend()?;
        let value = Self::encode(value)?;
        backend.set(&self.config.name, key, value).await
    }
}
