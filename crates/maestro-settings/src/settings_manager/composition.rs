//! Effective settings view composition for scoped readers.

use super::{BaseSettingsLayer, Settings, SettingsErrorKind, SettingsManager, SettingsScope};
use maestro_json::merge_one_level;
use serde_json::{Map, Value};

impl SettingsManager {
    /// Composes supplied scopes with locked inputs and final locks after stored decisions.
    pub(super) fn compose_effective_settings(
        &self,
        layer: Option<&BaseSettingsLayer>,
        initial: Map<String, Value>,
        scopes: &[(SettingsScope, &Map<String, Value>)],
    ) -> Map<String, Value> {
        let mut view = Settings(scopes.iter().fold(initial, |view, (_, scope)| {
            merge_one_level(view, (*scope).clone())
        }));
        let restore = |view: &mut Map<String, Value>| {
            let Some(layer) = layer else {
                return;
            };
            layer.apply_locks(view, |path| {
                let scope = scopes
                    .iter()
                    .rev()
                    .find(|(_, document)| overrides_path(document, path))
                    .map_or(SettingsScope::Global, |(scope, _)| *scope);
                self.queue.record(
                    scope,
                    SettingsErrorKind::StoredValueIgnored,
                    layer.lock_message(path, "Stored value ignored").into(),
                );
            });
        };
        restore(&mut view.0);
        if let Some(decider) = &self.decider {
            for message in decider.resolve(&mut view) {
                self.queue.record(
                    SettingsScope::Global,
                    SettingsErrorKind::StoredValueIgnored,
                    message.into(),
                );
            }
        }
        restore(&mut view.0);
        view.0
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

#[cfg(test)]
mod tests;
