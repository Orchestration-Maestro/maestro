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

/// A pending adapter operation, sendable between threads on native targets.
#[cfg(not(target_arch = "wasm32"))]
pub type AuthStorageFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
/// A pending adapter operation; browser futures stay on their thread.
#[cfg(target_arch = "wasm32")]
pub type AuthStorageFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;

/// Single awaited callback run by `with_lock_async`; each adapter documents what it holds meanwhile.
#[cfg(not(target_arch = "wasm32"))]
pub type AsyncLockUpdate<'a> =
    Box<dyn FnOnce(Option<String>) -> AuthStorageFuture<'a, LockUpdate> + Send + 'a>;
/// Single awaited callback run by `with_lock_async`; each adapter documents what it holds meanwhile.
#[cfg(target_arch = "wasm32")]
pub type AsyncLockUpdate<'a> =
    Box<dyn FnOnce(Option<String>) -> AuthStorageFuture<'a, LockUpdate> + 'a>;

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

    /// Read the text, await `update` with an owned copy, then store its replacement.
    ///
    /// Dropping the returned future releases any exclusion already acquired and
    /// never starts or resumes `update`; effects completed before the drop stay.
    ///
    /// # Errors
    /// Returns the adapter's acquisition, read or write failure, or the failure
    /// returned by `update`.
    fn with_lock_async<'a>(
        &'a self,
        update: AsyncLockUpdate<'a>,
    ) -> AuthStorageFuture<'a, Result<(), AuthStorageError>>;
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

    fn with_lock_async<'a>(
        &'a self,
        update: AsyncLockUpdate<'a>,
    ) -> AuthStorageFuture<'a, Result<(), AuthStorageError>> {
        Box::pin(async move {
            let current = self
                .value
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(next) = update(current).await? {
                *self.value.lock().unwrap_or_else(PoisonError::into_inner) = Some(next);
            }
            Ok(())
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use file::FileAuthStorageBackend;

/// Native credential file adapter.
#[cfg(not(target_arch = "wasm32"))]
mod file;
