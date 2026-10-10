//! Raw-text storage adapters: read under exclusion, run a callback, optionally write.
use super::types::AuthStorageError;
use std::sync::{Mutex, PoisonError};

mod thread_bound {
    //! Marks adapters that native callers share across threads.

    /// Native adapters are shared across threads.
    #[cfg(not(target_arch = "wasm32"))]
    pub trait ThreadBound: Send + Sync {}
    #[cfg(not(target_arch = "wasm32"))]
    impl<T: Send + Sync + ?Sized> ThreadBound for T {}

    /// Browser adapters run on one thread, so no bound applies.
    #[cfg(target_arch = "wasm32")]
    pub trait ThreadBound {}
    #[cfg(target_arch = "wasm32")]
    impl<T: ?Sized> ThreadBound for T {}
}
pub(super) use thread_bound::ThreadBound;

/// Replacement text, `None` to leave the stored text unchanged, or a failure.
pub type LockUpdate = Result<Option<String>, AuthStorageError>;

/// Raw credential text guarded by the adapter's exclusion.
pub trait AuthStorageBackend: ThreadBound {
    /// Run `update` once with the current text, then store its replacement.
    ///
    /// `None` means nothing has been stored. Acquisition and read failures
    /// return before `update` runs; an `update` failure supplies no
    /// replacement, though the adapter may already have initialized missing storage.
    ///
    /// # Errors
    /// Returns the adapter's acquisition, read, write or release failure, or
    /// the failure returned by `update`.
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
    ) -> Result<(), AuthStorageError>;
}

/// Text held in memory; `update` runs on an owned copy with nothing locked.
#[derive(Debug, Default)]
pub struct InMemoryAuthStorageBackend {
    /// Current text; `None` until first stored.
    value: Mutex<Option<String>>,
}

impl InMemoryAuthStorageBackend {
    /// Backend already holding `text`.
    pub(super) fn seeded(text: String) -> Self {
        Self {
            value: Mutex::new(Some(text)),
        }
    }
}

impl AuthStorageBackend for InMemoryAuthStorageBackend {
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
    ) -> Result<(), AuthStorageError> {
        let current = self
            .value
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(next) = update(current.as_deref())? {
            *self.value.lock().unwrap_or_else(PoisonError::into_inner) = Some(next);
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use file::FileAuthStorageBackend;

/// Native credential file adapter.
#[cfg(not(target_arch = "wasm32"))]
mod file;
