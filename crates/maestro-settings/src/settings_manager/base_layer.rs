//! Read-only settings supplied below the ordinary preference scopes.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use serde_json::{Map, Value};

use super::change::{Change, Edit};
use super::{Settings, SettingsManager, SettingsScope};

/// A publisher's accepted settings document with its locks and release metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaseSettingsLayer {
    /// Preferences below the raw global and project documents.
    pub settings: Settings,
    /// Dotted paths whose effective values come only from this document.
    pub locked: BTreeSet<String>,
    /// The publisher's source release, used in diagnostics.
    pub source: String,
    /// When the publisher accepted this release, as supplied by the publisher.
    pub recorded_at: String,
}

impl BaseSettingsLayer {
    /// Restores locked leaves and reports each overwritten stored value.
    pub(super) fn apply_locks(
        &self,
        settings: &mut Map<String, Value>,
        mut ignored: impl FnMut(&str),
    ) {
        for path in &self.locked {
            let edit = Edit::dotted(path);
            let value = edit.value_in(&self.settings.0).cloned();
            if edit.value_in(settings) != value.as_ref() {
                ignored(path);
            }
            edit.apply(settings, value);
        }
    }

    /// Formats a constraint with this layer's accepted publisher provenance.
    pub(super) fn lock_message(&self, path: &str, action: &str) -> String {
        format!(
            "{action}: `{path}` is locked by {} (recorded {}); starting Maestro refreshes the base layer",
            self.source, self.recorded_at
        )
    }
}

impl SettingsManager {
    /// Replaces or removes the base layer for a scope and queues its persisted record.
    /// Raw preferences and their pending edits are unchanged. Persistence failures
    /// are reported by [`Self::drain_errors`]; [`Self::flush`] waits for the save.
    /// Repeating a removal retries its persisted write after a failure.
    pub fn publish_base_layer(&mut self, scope: SettingsScope, layer: Option<BaseSettingsLayer>) {
        self.base_layers[scope as usize].clone_from(&layer);
        self.rebuild();
        self.queue.save_base_layer(scope, layer);
    }

    /// Returns the current read-only layer; a project layer takes precedence over global.
    #[must_use]
    pub fn get_base_layer(&self) -> Option<&BaseSettingsLayer> {
        self.base_layers[SettingsScope::Project as usize]
            .as_ref()
            .or(self.base_layers[SettingsScope::Global as usize].as_ref())
    }

    /// Reports a lock on this literal field/key path, any ancestor or any descendant.
    /// Sibling paths are independent. This query performs no mutation or I/O.
    #[must_use]
    pub fn is_key_path_locked(&self, field: &str, keys: &[&str]) -> bool {
        let path: Vec<&str> = std::iter::once(field).chain(keys.iter().copied()).collect();
        self.locked_path(&path).is_some()
    }

    /// Loads persisted layers once; reload retains the accepted publication.
    pub(super) fn load_base_layers(&mut self) {
        for scope in [SettingsScope::Global, SettingsScope::Project] {
            match self.storage.read_base_layer(scope) {
                Ok(layer) => self.base_layers[scope as usize] = layer,
                Err(error) => self.queue.record_error(scope, error),
            }
        }
    }

    /// Finds a lock on a path, one of its parents or one of its children.
    fn locked_path(&self, path: &[&str]) -> Option<&str> {
        self.get_base_layer()?.locked.iter().find_map(|locked| {
            let components: Vec<&str> = locked.split('.').collect();
            (components.starts_with(path) || path.starts_with(&components))
                .then_some(locked.as_str())
        })
    }

    /// Refuses a whole explicit request before it changes any manager state.
    pub(super) fn refuse_changes(&self, scope: SettingsScope, changes: &[Change]) -> bool {
        if let Err(message) = self.check_changes(changes) {
            self.queue.record_error(scope, message.into());
            return true;
        }
        false
    }

    /// Checks a derived edit without publishing a diagnostic or changing the queue.
    pub(super) fn check_changes(&self, changes: &[Change]) -> Result<(), String> {
        let message = changes
            .iter()
            .find_map(|change| self.locked_path(&change.components()))
            .and_then(|path| self.lock_message(path, "Edit refused"));
        message.map_or(Ok(()), Err)
    }

    /// Formats a constraint with the accepted publisher's provenance.
    fn lock_message(&self, path: &str, action: &str) -> Option<String> {
        self.get_base_layer()
            .map(|layer| layer.lock_message(path, action))
    }

    /// Clears native nonfinite flags for presentation values supplied by base locks.
    pub(super) fn apply_base_nan_locks(&mut self) {
        if self.locked_path(&["editorPaddingX"]).is_some() {
            self.effective_nan.editor_padding_x = false;
        }
        if self.locked_path(&["autocompleteMaxVisible"]).is_some() {
            self.effective_nan.autocomplete_max_visible = false;
        }
    }
}
