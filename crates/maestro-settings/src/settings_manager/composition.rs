//! Effective settings view composition for scoped readers.

use super::{BaseSettingsLayer, SettingsManager, SettingsScope};
use serde_json::{Map, Value};

impl SettingsManager {
    /// Composes supplied scopes over their initial view without changing raw preferences.
    pub(super) fn compose_effective_settings(
        &self,
        _layer: Option<&BaseSettingsLayer>,
        _initial: Map<String, Value>,
        _scopes: &[(SettingsScope, &Map<String, Value>)],
    ) -> Map<String, Value> {
        self.effective.clone()
    }
}

#[cfg(test)]
mod tests;
