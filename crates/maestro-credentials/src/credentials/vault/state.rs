//! Accepted credential state, its reload and per-provider persistence.
use super::backend::{AuthStorageBackend, InMemoryAuthStorageBackend, ThreadBound};
use super::records::{
    AuthCredential, AuthStorageData, AuthStorageError, decode, parse_storage_data, stored_record,
};
use maestro_models::OAuthError;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// A key lookup consulted after stored and environment keys.
pub(super) trait FallbackResolver:
    Fn(&str) -> Result<Option<String>, OAuthError> + ThreadBound
{
}
impl<F: Fn(&str) -> Result<Option<String>, OAuthError> + ThreadBound> FallbackResolver for F {}

/// Accepted state; locked only for short steps, never across callbacks.
#[derive(Default)]
pub(super) struct State {
    /// Accepted records in document order, unknown records included.
    pub(super) data: Map<String, Value>,
    /// Runtime key overrides, never persisted.
    pub(super) runtime_overrides: HashMap<String, String>,
    /// Lookup consulted after stored and environment keys.
    pub(super) fallback_resolver: Option<Arc<dyn FallbackResolver>>,
    /// Whether the last load failed; only a successful reload or locked refresh reread clears it.
    pub(super) load_error: bool,
    /// Failures in recording order.
    pub(super) errors: Vec<AuthStorageError>,
}

impl State {
    /// Accept stored records and clear the recorded load failure.
    pub(super) fn accept(&mut self, data: Map<String, Value>) {
        self.data = data;
        self.load_error = false;
    }
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
    ///
    /// Every store installs the built-in login providers as the registry's base layer, so
    /// login, refresh and a catalog built over this store find them without another call.
    #[must_use]
    pub fn from_storage(storage: impl AuthStorageBackend + 'static) -> Self {
        maestro_providers::install_built_in_providers();
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
        maestro_lock::lock(&self.state)
    }

    /// Adopt any readable stored document without clearing a recorded load failure.
    ///
    /// Waits for exclusion on the backend's schedule. An acquisition, read or parse failure
    /// keeps the accepted records and records nothing.
    pub(super) fn latest(&self) -> MutexGuard<'_, State> {
        let read = self.storage.read_waiting();
        let mut state = self.state();
        if let Ok(seen) = read
            && let Ok(data) = parse_storage_data(seen.as_deref())
        {
            state.data = data;
        }
        state
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
        resolver: impl Fn(&str) -> Result<Option<String>, OAuthError> + ThreadBound + 'static,
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
            Ok(()) => state.accept(loaded),
            Err(error) => {
                state.load_error = true;
                state.errors.push(error);
            }
        }
    }

    /// Merge one provider change into the stored document unless a load failed.
    fn persist_provider_change(&self, provider: &str, mut record: Option<Value>) {
        let mut written = None;
        let outcome = self.storage.with_lock(&mut |current| {
            let mut merged = parse_storage_data(current)?;
            match record.take() {
                Some(record) => merged.insert(provider.to_owned(), record),
                None => merged.shift_remove(provider),
            };
            let text = format!("{:#}", Value::Object(merged.clone()));
            written = Some(merged);
            Ok(Some(text))
        });
        let mut state = self.state();
        match (outcome, written) {
            (Ok(()), Some(merged)) => state.accept(merged),
            (Err(error), _) => state.errors.push(error),
            (Ok(()), None) => {}
        }
    }

    /// Typed copy of the stored credential, when the record is a complete known one.
    ///
    /// Rereads the stored document first.
    #[must_use]
    pub fn get(&self, provider: &str) -> Option<AuthCredential> {
        self.latest().data.get(provider).and_then(decode)
    }

    /// Store a credential; a failed write stays in accepted memory until the next successful read.
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

    /// Remove a credential; a failed write stays in accepted memory until the next successful read.
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

    /// Stored provider names in accepted order, after rereading the stored document.
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        self.latest().data.keys().cloned().collect()
    }

    /// Whether any record is stored for `provider`, after rereading the stored document.
    #[must_use]
    pub fn has(&self, provider: &str) -> bool {
        self.latest().data.contains_key(provider)
    }

    /// Typed copies of every complete known stored credential, in accepted order, after
    /// rereading the stored document.
    #[must_use]
    pub fn get_all(&self) -> AuthStorageData {
        self.latest()
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
