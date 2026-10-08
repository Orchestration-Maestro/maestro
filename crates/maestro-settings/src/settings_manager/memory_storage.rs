//! In-memory raw-text storage.

use std::sync::{Mutex, MutexGuard};

use super::preferences::{SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate};

/// Storage that keeps each scope's raw text in memory. The callback runs on an
/// owned copy of the text with no lock held, so it may use this storage again;
/// the last write wins.
#[derive(Debug, Default)]
pub struct InMemorySettingsStorage {
    /// Raw text per scope, in `[global, project]` order.
    texts: Mutex<[Option<String>; 2]>,
}

impl InMemorySettingsStorage {
    /// Creates storage in which neither scope has ever been written.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates storage whose global scope already holds `text`.
    pub(super) fn seeded(text: String) -> Self {
        Self {
            texts: Mutex::new([Some(text), None]),
        }
    }

    /// Locks the texts for a single read or write, never across caller code.
    fn locked(&self) -> MutexGuard<'_, [Option<String>; 2]> {
        self.texts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SettingsStorage for InMemorySettingsStorage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let current = self.locked()[scope as usize].clone();
        if let Some(next) = update(current.as_deref())? {
            self.locked()[scope as usize] = Some(next);
        }
        Ok(())
    }
}
