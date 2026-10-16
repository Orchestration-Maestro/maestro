//! Raw operations scoped to one owned browser transaction.
use super::{browser_error, put, read, remove};
use std::io;
use wasm_bindgen::JsValue;
use web_sys::IdbTransaction;

/// One retained native transaction; retention does not extend browser activity.
pub struct IndexedDBStorageTransaction {
    /// The same transaction supplies every operation's object store.
    native: IdbTransaction,
}
impl IndexedDBStorageTransaction {
    /// Wrap the single transaction admitted by the backend.
    pub(super) fn new(native: IdbTransaction) -> Self {
        Self { native }
    }
    /// Read a native value, mapping only undefined and null to absence.
    /// # Errors
    /// Returns native transaction, store or request failures.
    pub async fn get(&self, store_name: &str, key: &str) -> io::Result<Option<JsValue>> {
        let store = self
            .native
            .object_store(store_name)
            .map_err(|error| browser_error(&error))?;
        read(&store, key).await
    }
    /// Put a raw value using this transaction's live store key path.
    /// # Errors
    /// Returns native transaction, store, key, clone or request failures.
    pub async fn set(&self, store_name: &str, key: &str, value: JsValue) -> io::Result<()> {
        let store = self
            .native
            .object_store(store_name)
            .map_err(|error| browser_error(&error))?;
        put(&store, key, value).await
    }
    /// Delete a key on this transaction without opening a new one.
    /// # Errors
    /// Returns native transaction, store, key or request failures.
    pub async fn delete(&self, store_name: &str, key: &str) -> io::Result<()> {
        let store = self
            .native
            .object_store(store_name)
            .map_err(|error| browser_error(&error))?;
        remove(&store, key).await
    }
}
