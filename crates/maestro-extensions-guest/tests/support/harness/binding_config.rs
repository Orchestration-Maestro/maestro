//! The script form of keybinding bindings, shared by the author harness and the terminal
//! keybinding witness.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::KeybindingsConfig;
use super::binding_json::keys_json;

/// The script form of bindings.
pub fn bindings_json(config: &KeybindingsConfig) -> Value {
    json!(
        config
            .iter()
            .map(|(id, keys)| json!([id, keys.as_ref().map(keys_json)]))
            .collect::<Vec<_>>()
    )
}
