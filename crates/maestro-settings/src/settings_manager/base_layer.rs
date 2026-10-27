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

impl SettingsManager {
    /// Replaces or removes the base layer for a scope and queues its persisted record.
    /// Raw preferences and their pending edits are unchanged. Persistence failures
    /// are reported by [`Self::drain_errors`]; [`Self::flush`] waits for the save.
    pub fn publish_base_layer(&mut self, scope: SettingsScope, layer: Option<BaseSettingsLayer>) {
        if layer.is_none() && self.base_layers[scope as usize].is_none() {
            return;
        }
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
        let Some(locked) = changes
            .iter()
            .find_map(|change| self.locked_path(&change.components()))
        else {
            return false;
        };
        self.record_lock(scope, locked, "Edit refused");
        true
    }

    /// Reports a constraint with the publisher's persisted provenance.
    fn record_lock(&self, scope: SettingsScope, path: &str, action: &str) {
        if let Some(layer) = self.get_base_layer() {
            let message = format!(
                "{action}: `{path}` is locked by {} (recorded {}); starting Maestro refreshes the base layer",
                layer.source, layer.recorded_at
            );
            self.queue.record_error(scope, message.into());
        }
    }

    /// Restores locked leaves after the ordinary merges without editing raw scopes.
    pub(super) fn apply_base_locks(&mut self) {
        let Some(layer) = self.get_base_layer().cloned() else {
            return;
        };
        for path in &layer.locked {
            let edit = Edit::dotted(path);
            let value = edit.value_in(&layer.settings.0).cloned();
            let scope = if overrides_path(&self.project.settings, path) {
                SettingsScope::Project
            } else {
                SettingsScope::Global
            };
            if edit.value_in(&self.effective) != value.as_ref() {
                self.record_lock(scope, path, "Stored value ignored");
            }
            edit.apply(&mut self.effective, value);
        }
        if self.locked_path(&["editorPaddingX"]).is_some() {
            self.effective_nan.editor_padding_x = false;
        }
        if self.locked_path(&["autocompleteMaxVisible"]).is_some() {
            self.effective_nan.autocomplete_max_visible = false;
        }
    }
}

/// Whether the one-level merge lets this document replace a locked leaf.
fn overrides_path(document: &Map<String, Value>, path: &str) -> bool {
    let mut keys = path.split('.');
    let Some(field) = keys.next().and_then(|key| document.get(key)) else {
        return false;
    };
    match (field, keys.next()) {
        (Value::Object(object), Some(key)) => object.contains_key(key),
        _ => true,
    }
}
