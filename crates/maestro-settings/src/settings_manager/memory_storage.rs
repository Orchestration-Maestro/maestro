//! In-memory raw-text storage.

use std::sync::{Mutex, MutexGuard};

use super::BaseSettingsLayer;
use super::preferences::{SettingsScope, SettingsStorage, SettingsStorageError, SettingsUpdate};

/// Storage that keeps each scope's raw text in memory. The callback runs on an
/// owned copy of the text with no lock held, so it may use this storage again;
/// the last write wins.
#[derive(Debug, Default)]
pub struct InMemorySettingsStorage {
    /// Raw text and base records, guarded only during individual reads and writes.
    state: Mutex<State>,
}

/// Stored values in global/project order.
#[derive(Debug, Default)]
struct State {
    /// Raw preference text per scope.
    texts: [Option<String>; 2],
    /// Accepted base layers per scope.
    layers: [Option<BaseSettingsLayer>; 2],
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
            state: Mutex::new(State {
                texts: [Some(text), None],
                ..State::default()
            }),
        }
    }

    /// Locks the state for a single read or write, never across caller code.
    fn locked(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SettingsStorage for InMemorySettingsStorage {
    fn read_base_layer(
        &self,
        scope: SettingsScope,
    ) -> Result<Option<BaseSettingsLayer>, SettingsStorageError> {
        Ok(self.locked().layers[scope as usize].clone())
    }

    fn write_base_layer(
        &self,
        scope: SettingsScope,
        layer: Option<&BaseSettingsLayer>,
    ) -> Result<(), SettingsStorageError> {
        self.locked().layers[scope as usize] = layer.cloned();
        Ok(())
    }

    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let current = self.locked().texts[scope as usize].clone();
        if let Some(next) = update(current.as_deref())? {
            self.locked().texts[scope as usize] = Some(next);
        }
        Ok(())
    }
}
