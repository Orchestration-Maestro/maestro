//! In-memory raw-text storage.

use std::sync::Mutex;

use super::preferences::{SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate};

/// Storage that keeps each scope's raw text in memory.
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
}

impl SettingsStorage for InMemorySettingsStorage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let slot = scope as usize;
        let current = self
            .texts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[slot]
            .clone();
        let next = update(current.as_deref())?;
        if next.is_some() {
            self.texts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)[slot] = next;
        }
        Ok(())
    }
}
