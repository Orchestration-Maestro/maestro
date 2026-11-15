//! The script form of keybinding keys, shared by the author harness and the terminal
//! keybinding witness.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::KeybindingKeys;

/// The script form of one binding's keys.
pub fn keys_json(keys: &KeybindingKeys) -> Value {
    match keys {
        KeybindingKeys::Single(key) => json!(key),
        KeybindingKeys::Multiple(keys) => json!(keys),
    }
}
