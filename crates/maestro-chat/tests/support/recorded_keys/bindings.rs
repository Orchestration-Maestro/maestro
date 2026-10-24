//! Recorded scalar/list overrides and active-manager configuration.
use maestro_app::presentation_data::bindings::KEYBINDINGS;
use maestro_tui::{
    KeybindingKeys, KeybindingsConfig, KeybindingsManager, TUI_KEYBINDINGS, get_keybindings,
    set_keybindings,
};
use serde::Deserialize;
use std::collections::BTreeMap;
/// Accepted caller binding shapes.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Binding {
    /// One key.
    Single(String),
    /// An explicit list, including a disabled empty list.
    Multiple(Vec<String>),
}
/// Replace the active manager with recorded caller overrides of the application defaults.
pub fn replace(values: &BTreeMap<String, Binding>) {
    set_keybindings(KeybindingsManager::new(
        KEYBINDINGS.clone(),
        bindings(values),
    ));
}
/// Replace the active manager with recorded caller overrides of the toolkit defaults alone.
pub fn replace_toolkit(values: &BTreeMap<String, Binding>) {
    set_keybindings(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        bindings(values),
    ));
}
/// Mutate the currently active manager's overrides.
pub fn mutate(values: &BTreeMap<String, Binding>) {
    get_keybindings().set_user_bindings(bindings(values));
}
/// Uses the toolkit's original scalar/list override shapes.
pub fn bindings(values: &BTreeMap<String, Binding>) -> KeybindingsConfig {
    values
        .iter()
        .map(|(id, value)| {
            let keys = match value {
                Binding::Single(key) => KeybindingKeys::Single(key.clone()),
                Binding::Multiple(keys) => KeybindingKeys::Multiple(keys.clone()),
            };
            (id.clone(), Some(keys))
        })
        .collect()
}
