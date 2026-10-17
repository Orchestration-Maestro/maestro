//! Effective shortcut classification and ordered owner selection.
use super::ExtensionRunner;
use crate::{
    ExtensionExecution,
    types::{ExtensionShortcut, read, write},
};
use indexmap::{IndexMap, IndexSet};
use maestro_resources::{DiagnosticType, ResourceDiagnostic};
use maestro_tui::{KeyId, KeybindingKeys, KeybindingsConfig};
use std::sync::{Arc, RwLock};

/// Editor-global actions whose effective keys cannot be overridden.
const RESERVED: [&str; 17] = [
    "app.interrupt",
    "app.clear",
    "app.exit",
    "app.suspend",
    "app.thinking.cycle",
    "app.model.cycleForward",
    "app.model.cycleBackward",
    "app.model.select",
    "app.tools.expand",
    "app.thinking.toggle",
    "app.editor.external",
    "app.message.followUp",
    "tui.input.submit",
    "tui.select.confirm",
    "tui.select.cancel",
    "tui.input.copy",
    "tui.editor.deleteToLineEnd",
];
/// Classifies supplied effective keys without selecting defaults.
fn builtin_keybindings(config: &KeybindingsConfig) -> (IndexSet<KeyId>, IndexMap<KeyId, String>) {
    let mut reserved = IndexSet::new();
    let mut ordinary = IndexMap::new();
    for (action, keys) in config {
        let keys: &[KeyId] = match keys {
            Some(KeybindingKeys::Single(key)) => std::slice::from_ref(key),
            Some(KeybindingKeys::Multiple(keys)) => keys,
            None => &[],
        };
        for key in keys {
            let key = key.to_lowercase();
            if RESERVED.contains(&action.as_str()) {
                reserved.insert(key);
            } else {
                ordinary.insert(key, action.clone());
            }
        }
    }
    (reserved, ordinary)
}
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Selects current shortcuts using supplied effective bindings.
    ///
    /// Reserved keys are blocked; ordinary and extension collisions warn but
    /// retain the later extension in the first key's slot. Warnings are reported
    /// outside locks only when no UI context is installed.
    #[must_use]
    pub fn get_shortcuts(
        &self,
        resolved_keybindings: &KeybindingsConfig,
    ) -> IndexMap<KeyId, E::Shared<RwLock<ExtensionShortcut<E>>>> {
        let diagnostics = Arc::new(RwLock::new(Vec::new()));
        *write(&self.shortcut_diagnostics) = Arc::clone(&diagnostics);
        let (reserved, ordinary) = builtin_keybindings(resolved_keybindings);
        let registrations: Vec<_> = read(&self.extensions)
            .iter()
            .flat_map(|owner| {
                read(owner)
                    .shortcuts
                    .iter()
                    .map(|(key, shortcut)| (key.clone(), shortcut.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut selected = IndexMap::<KeyId, E::Shared<RwLock<ExtensionShortcut<E>>>>::new();
        for (key, shortcut) in registrations {
            let normalized = key.to_lowercase();
            let path = read(&shortcut).extension_path.clone();
            if reserved.contains(&normalized) {
                self.add_shortcut_diagnostic(&diagnostics, format!("Extension shortcut '{key}' from {path} conflicts with built-in shortcut. Skipping."), &path);
                continue;
            }
            if let Some(action) = ordinary.get(&normalized) {
                self.add_shortcut_diagnostic(&diagnostics, format!("Extension shortcut conflict: '{key}' is built-in shortcut for {action} and {path}. Using {path}."), &path);
            }
            if let Some(previous) = selected.get(&normalized) {
                let previous_path = read(previous).extension_path.clone();
                self.add_shortcut_diagnostic(&diagnostics, format!("Extension shortcut conflict: '{key}' registered by both {previous_path} and {path}. Using {path}."), &path);
            }
            selected.insert(normalized, shortcut);
        }
        selected
    }
    /// Publishes one warning before invoking the caller's reporter without locks.
    fn add_shortcut_diagnostic(
        &self,
        diagnostics: &Arc<RwLock<Vec<ResourceDiagnostic>>>,
        message: String,
        path: &str,
    ) {
        let report = (!self.has_ui()).then(|| format!("{message}\n"));
        write(diagnostics).push(ResourceDiagnostic {
            r#type: DiagnosticType::Warning,
            message,
            path: Some(path.into()),
            collision: None,
        });
        if let Some(report) = report {
            (self.warning_reporter)(&report);
        }
    }
}
