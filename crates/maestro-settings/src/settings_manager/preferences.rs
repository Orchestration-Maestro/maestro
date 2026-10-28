//! Raw preference data, the storage seam and the scoped error type.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::BaseSettingsLayer;

pub(super) mod thread_bound {
    //! Marks storage adapters that the queue worker may own.

    /// Native adapters must be shareable with the queue worker thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub trait StorageThreadBound: Send + Sync {}
    #[cfg(not(target_arch = "wasm32"))]
    impl<T: Send + Sync + ?Sized> StorageThreadBound for T {}

    /// Browser adapters run on one thread, so no bound applies.
    #[cfg(target_arch = "wasm32")]
    pub trait StorageThreadBound {}
    #[cfg(target_arch = "wasm32")]
    impl<T: ?Sized> StorageThreadBound for T {}
}

/// An owned global or project preference document: a standard JSON object that
/// keeps unknown and wrong-typed members untouched.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Settings(pub Map<String, Value>);

/// Which preference file a storage operation addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsScope {
    /// The user-wide preference file in the agent directory.
    Global,
    /// The per-project preference file in the project configuration directory.
    Project,
}

impl SettingsScope {
    /// The scope's name as messages spell it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }
}

/// The cause carried by a storage failure.
pub type SettingsStorageError = Box<dyn std::error::Error + Send + Sync>;

/// The outcome of a storage callback: replacement text, `None` to leave the
/// stored text alone, or a failure.
pub type SettingsUpdate = Result<Option<String>, SettingsStorageError>;

/// A failure attributed to the scope whose load or save produced it.
#[derive(Debug)]
pub struct SettingsError {
    /// The scope whose operation failed.
    pub scope: SettingsScope,
    /// The underlying parser, I/O or adapter failure.
    pub error: SettingsStorageError,
}

/// The shared storage handle that managers and their queue worker hold.
#[cfg(not(target_arch = "wasm32"))]
pub type SettingsStorageHandle = std::sync::Arc<dyn SettingsStorage>;
/// The shared storage handle that managers and their queue worker hold.
#[cfg(target_arch = "wasm32")]
pub type SettingsStorageHandle = std::rc::Rc<dyn SettingsStorage>;

/// Wraps a storage adapter in the shared handle of this target.
pub(super) fn share(storage: impl SettingsStorage + 'static) -> SettingsStorageHandle {
    #[cfg(not(target_arch = "wasm32"))]
    return std::sync::Arc::new(storage);
    #[cfg(target_arch = "wasm32")]
    return std::rc::Rc::new(storage);
}

/// Raw-text storage for one scope at a time.
pub trait SettingsStorage: thread_bound::StorageThreadBound {
    /// Reads the last published base layer for this scope.
    /// Adapters without base-layer persistence return no layer by default.
    ///
    /// # Errors
    ///
    /// Returns a storage or decoding failure without changing preferences.
    fn read_base_layer(
        &self,
        _scope: SettingsScope,
    ) -> Result<Option<BaseSettingsLayer>, SettingsStorageError> {
        Ok(None)
    }

    /// Replaces or clears the persisted base record without editing raw preferences.
    /// The default retains no record; adapters can opt into persistence.
    ///
    /// # Errors
    ///
    /// Returns an acquire, encoding or write failure.
    fn write_base_layer(
        &self,
        _scope: SettingsScope,
        _layer: Option<&BaseSettingsLayer>,
    ) -> Result<(), SettingsStorageError> {
        Ok(())
    }

    /// Runs `update` exactly once, synchronously, with the current raw text
    /// (`None` when the scope has never been written), unless the adapter fails
    /// to acquire its exclusion or to read first; it then returns that failure
    /// without calling `update`.
    ///
    /// `Ok(None)` leaves the stored text untouched, `Ok(Some(text))` replaces it
    /// and `Err` propagates without writing. For an existing file the file adapter
    /// holds its lock from before the read until after the write. For a missing
    /// file it runs `update` before creating any directory or lock, so concurrent
    /// first replacements need [`Self::with_write_lock`] instead. The in-memory
    /// adapter holds nothing while `update` runs, allowing storage re-entry.
    ///
    /// # Errors
    ///
    /// Returns the callback's failure or the adapter's own acquire, read or
    /// write failure.
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError>;

    /// Runs a raw-text update with explicit write intent.
    ///
    /// The default delegates once to [`Self::with_lock`], preserving the supplied
    /// adapter's callback, exclusion and re-entry policy. Native decorators must
    /// forward this operation to retain the file adapter's write exclusion.
    /// The file adapter creates the parent as needed and acquires its stable
    /// sidecar before reading and invoking `update` exactly once. It releases
    /// exclusion on every outcome; acquire/read failures skip the callback.
    /// `Ok(None)` and callback failure leave settings bytes untouched but may
    /// leave a newly created parent and sidecar, never a new settings file.
    ///
    /// # Errors
    ///
    /// Returns the callback's failure or the adapter's acquire, read or write failure.
    fn with_write_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        self.with_lock(scope, update)
    }
}
