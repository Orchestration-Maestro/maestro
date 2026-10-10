//! Accepted credential state, its reload and per-provider persistence.
use super::backend::{AuthStorageBackend, InMemoryAuthStorageBackend, ThreadBound};
use super::types::{
    AuthCredential, AuthStorageData, AuthStorageError, decode, parse_storage_data, stored_record,
};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// A key lookup consulted after stored and environment keys.
pub(super) trait FallbackResolver: Fn(&str) -> Option<String> + ThreadBound {}
impl<F: Fn(&str) -> Option<String> + ThreadBound> FallbackResolver for F {}

/// Accepted state; locked only for short steps, never across callbacks.
#[derive(Default)]
pub(super) struct State {
    /// Accepted records in document order, unknown records included.
    pub(super) data: Map<String, Value>,
    /// Runtime key overrides, never persisted.
    pub(super) runtime_overrides: HashMap<String, String>,
    /// Lookup consulted after stored and environment keys.
    pub(super) fallback_resolver: Option<Arc<dyn FallbackResolver>>,
    /// Whether the last load failed; writes are skipped until a reload succeeds.
    pub(super) load_error: bool,
    /// Failures in recording order.
    pub(super) errors: Vec<AuthStorageError>,
}

/// Accepted stored credentials over a raw-text backend.
pub struct AuthStorage {
    /// Raw-text storage.
    pub(super) storage: Box<dyn AuthStorageBackend>,
    /// Accepted state.
    state: Mutex<State>,
    /// Current epoch milliseconds; tests substitute a fixed reading.
    pub(super) clock: fn() -> f64,
}

impl AuthStorage {
    /// Store credentials in the file at `auth_path`, loading it once.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn create(auth_path: &str) -> Self {
        Self::from_storage(super::backend::FileAuthStorageBackend::new(auth_path))
    }

    /// Accept a storage backend, loading it once.
    #[must_use]
    pub fn from_storage(storage: impl AuthStorageBackend + 'static) -> Self {
        let auth_storage = Self {
            storage: Box::new(storage),
            state: Mutex::default(),
            clock: maestro_models::timestamp_now,
        };
        auth_storage.reload();
        auth_storage
    }

    /// Replace the clock read for expiry decisions.
    #[cfg(test)]
    pub(super) fn with_clock(mut self, clock: fn() -> f64) -> Self {
        self.clock = clock;
        self
    }

    /// Store credentials in memory, seeded with `data`.
    #[must_use]
    pub fn in_memory(data: AuthStorageData) -> Self {
        let document: Map<String, Value> = data
            .into_iter()
            .map(|(provider, credential)| (provider, stored_record(credential)))
            .collect();
        Self::from_storage(InMemoryAuthStorageBackend::seeded(format!(
            "{:#}",
            Value::Object(document)
        )))
    }

    /// Lock the accepted state for one short step.
    pub(super) fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Set a runtime key override; it is never persisted.
    pub fn set_runtime_api_key(&self, provider: &str, api_key: &str) {
        self.state()
            .runtime_overrides
            .insert(provider.to_owned(), api_key.to_owned());
    }

    /// Remove a runtime key override.
    pub fn remove_runtime_api_key(&self, provider: &str) {
        self.state().runtime_overrides.remove(provider);
    }

    /// Set the key lookup consulted after stored and environment keys.
    pub fn set_fallback_resolver(
        &self,
        resolver: impl Fn(&str) -> Option<String> + ThreadBound + 'static,
    ) {
        self.state().fallback_resolver = Some(Arc::new(resolver));
    }

    /// Reload accepted data; a failure keeps it and blocks writes until a later success.
    pub fn reload(&self) {
        let mut loaded = Map::new();
        let outcome = self.storage.with_lock(&mut |current| {
            loaded = parse_storage_data(current)?;
            Ok(None)
        });
        let mut state = self.state();
        match outcome {
            Ok(()) => {
                state.data = loaded;
                state.load_error = false;
            }
            Err(error) => {
                state.load_error = true;
                state.errors.push(error);
            }
        }
    }

    /// Merge one provider change into the stored document unless a load failed.
    fn persist_provider_change(&self, provider: &str, mut record: Option<Value>) {
        let outcome = self.storage.with_lock(&mut |current| {
            let mut merged = parse_storage_data(current)?;
            match record.take() {
                Some(record) => merged.insert(provider.to_owned(), record),
                None => merged.shift_remove(provider),
            };
            Ok(Some(format!("{:#}", Value::Object(merged))))
        });
        if let Err(error) = outcome {
            self.state().errors.push(error);
        }
    }

    /// Typed copy of the stored credential, when the record is a complete known one.
    #[must_use]
    pub fn get(&self, provider: &str) -> Option<AuthCredential> {
        self.state().data.get(provider).and_then(decode)
    }

    /// Store a credential; accepted memory changes even when persistence fails.
    pub fn set(&self, provider: &str, credential: AuthCredential) {
        let record = stored_record(credential);
        let load_error = {
            let mut state = self.state();
            state.data.insert(provider.to_owned(), record.clone());
            state.load_error
        };
        if !load_error {
            self.persist_provider_change(provider, Some(record));
        }
    }

    /// Remove a stored credential; accepted memory changes even when persistence fails.
    pub fn remove(&self, provider: &str) {
        let load_error = {
            let mut state = self.state();
            state.data.shift_remove(provider);
            state.load_error
        };
        if !load_error {
            self.persist_provider_change(provider, None);
        }
    }

    /// Stored provider names in accepted order.
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        self.state().data.keys().cloned().collect()
    }

    /// Whether any record is stored for `provider`.
    #[must_use]
    pub fn has(&self, provider: &str) -> bool {
        self.state().data.contains_key(provider)
    }

    /// Typed copies of every complete known stored credential, in accepted order.
    #[must_use]
    pub fn get_all(&self) -> AuthStorageData {
        self.state()
            .data
            .iter()
            .filter_map(|(provider, record)| Some((provider.clone(), decode(record)?)))
            .collect()
    }

    /// Take the recorded failures, oldest first.
    pub fn drain_errors(&self) -> Vec<AuthStorageError> {
        std::mem::take(&mut self.state().errors)
    }

    /// Remove the local stored credential.
    pub fn logout(&self, provider: &str) {
        self.remove(provider);
    }
}
