//! Read-only settings supplied below the ordinary preference scopes.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use serde_json::{Map, Value};

use super::change::{Change, Edit};
use super::{Settings, SettingsErrorKind, SettingsManager, SettingsScope};

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
    /// Reports a lock on this literal field/key path, any ancestor or any descendant.
    /// Sibling paths are independent. This query performs no mutation or I/O.
    #[must_use]
    pub fn is_key_path_locked(&self, field: &str, keys: &[&str]) -> bool {
        let path: Vec<&str> = std::iter::once(field).chain(keys.iter().copied()).collect();
        self.locked_path(&path).is_some()
    }

    /// Restores current locks in a previously accepted plain settings document.
    pub fn restore_locked_settings(&self, settings: &mut Settings) {
        self.apply_locks(&mut settings.0, |_| {});
    }

    /// Finds a lock on a path, one of its parents or one of its children.
    fn locked_path(&self, path: &[&str]) -> Option<&str> {
        self.locked.iter().find_map(|locked| {
            let components: Vec<&str> = locked.split('.').collect();
            (components.starts_with(path) || path.starts_with(&components))
                .then_some(locked.as_str())
        })
    }

    /// Restores locked leaves and reports each overwritten stored value.
    pub(super) fn apply_locks(
        &self,
        settings: &mut Map<String, Value>,
        mut ignored: impl FnMut(&str),
    ) {
        for path in &self.locked {
            let edit = Edit::dotted(path);
            let value = edit.value_in(&self.settings.0).cloned();
            let malformed = value.is_none() && edit.remove_malformed_ancestor(settings);
            if malformed || edit.value_in(settings) != value.as_ref() {
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
            .or(self.get_global_base_layer())
    }

    /// Returns the cached global layer, ignoring any project layer.
    /// This performs no I/O; [`Self::effective_global_settings`] refreshes the record.
    #[must_use]
    pub fn get_global_base_layer(&self) -> Option<&BaseSettingsLayer> {
        self.base_layers[SettingsScope::Global as usize].as_ref()
    }

    /// Rereads the persisted global base layer and resolves accepted user preferences.
    /// Project settings, project layers and runtime overrides never enter this result.
    /// The registered decider resolves the stored view, and the layer's locks are
    /// restored before and after it runs. Without a base layer or decider this equals
    /// [`Self::get_global_settings`]. Read failures retain the accepted layer and are
    /// reported by [`Self::drain_errors`].
    #[must_use]
    pub fn effective_global_settings(&mut self) -> Settings {
        match self.storage.read_base_layer(SettingsScope::Global) {
            Ok(layer) => self.base_layers[SettingsScope::Global as usize] = layer,
            Err(error) => self.queue.record_error(SettingsScope::Global, error),
        }
        self.compose_global_settings(&self.global.settings)
    }

    /// Resolves the cached global base layer with an empty user scope, the global view
    /// when no user document is readable. It follows [`Self::effective_global_settings`]
    /// without rereading the layer: the decider runs and the locks are restored after it.
    #[must_use]
    pub fn global_base_settings(&self) -> Settings {
        self.compose_global_settings(&Map::new())
    }

    /// Composes the cached global base layer with one global user scope.
    fn compose_global_settings(&self, user: &Map<String, Value>) -> Settings {
        let layer = self.get_global_base_layer();
        Settings(self.compose_effective_settings(
            layer,
            layer.map_or_else(Map::new, |layer| layer.settings.0.clone()),
            &[(SettingsScope::Global, user)],
        ))
    }

    /// Queries the current layer, with a project layer taking precedence.
    /// See [`BaseSettingsLayer::is_key_path_locked`] for path semantics.
    #[must_use]
    pub fn is_key_path_locked(&self, field: &str, keys: &[&str]) -> bool {
        self.get_base_layer()
            .is_some_and(|layer| layer.is_key_path_locked(field, keys))
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

    /// Finds a lock in the currently active layer.
    fn locked_path(&self, path: &[&str]) -> Option<&str> {
        self.get_base_layer()?.locked_path(path)
    }

    /// Refuses a scoped edit against only the layer governing that scope.
    pub(super) fn refuse_changes(&self, scope: SettingsScope, changes: &[Change]) -> bool {
        let layer = match scope {
            SettingsScope::Global => self.base_layers[scope as usize].as_ref(),
            SettingsScope::Project => self.get_base_layer(),
        };
        self.refuse_layer_changes(scope, layer, changes)
    }

    /// Refuses a whole explicit request before it changes any manager state.
    pub(super) fn refuse_layer_changes(
        &self,
        scope: SettingsScope,
        layer: Option<&BaseSettingsLayer>,
        changes: &[Change],
    ) -> bool {
        let Some(layer) = layer else {
            return false;
        };
        let Some(locked) = changes
            .iter()
            .find_map(|change| layer.locked_path(&change.components()))
        else {
            return false;
        };
        self.queue.record(
            scope,
            SettingsErrorKind::Failure,
            layer.lock_message(locked, "Edit refused").into(),
        );
        true
    }

    /// Checks derived edits against the layer governing their raw scope.
    pub(super) fn check_changes(
        &self,
        scope: SettingsScope,
        changes: &[Change],
    ) -> Result<(), String> {
        let layer = match scope {
            SettingsScope::Global => self.get_global_base_layer(),
            SettingsScope::Project => self.get_base_layer(),
        };
        let message = layer.and_then(|layer| {
            changes
                .iter()
                .find_map(|change| layer.locked_path(&change.components()))
                .map(|path| layer.lock_message(path, "Edit refused"))
        });
        message.map_or(Ok(()), Err)
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
