//! Keybindings requests through the generated host bindings.

use crate::bindings::maestro::extension::host;
use crate::component_adapter::{KeybindingsAnswer, KeybindingsRequest};
use crate::types::ExtensionResult;
use crate::{KeybindingConflict, KeybindingDefinition, KeybindingKeys, KeybindingsConfig};

/// Sends one request to a host keybindings manager.
pub(super) fn request(
    manager: &host::KeybindingsManager,
    request: KeybindingsRequest<'_>,
) -> ExtensionResult<KeybindingsAnswer> {
    Ok(match request {
        KeybindingsRequest::Matches(data, id) => {
            KeybindingsAnswer::Matched(manager.matches(data, id)?)
        }
        KeybindingsRequest::Keys(id) => KeybindingsAnswer::Keys(manager.get_keys(id)?),
        KeybindingsRequest::Definition(id) => {
            KeybindingsAnswer::Definition(manager.get_definition(id)?.map(definition))
        }
        KeybindingsRequest::Conflicts => KeybindingsAnswer::Conflicts(
            manager
                .get_conflicts()?
                .into_iter()
                .map(|conflict| KeybindingConflict {
                    key: conflict.key,
                    keybindings: conflict.keybindings,
                })
                .collect(),
        ),
        KeybindingsRequest::SetUserBindings(config) => {
            manager.set_user_bindings(&wire_config(config))?;
            KeybindingsAnswer::Done
        }
        KeybindingsRequest::UserBindings => {
            KeybindingsAnswer::Config(config(manager.get_user_bindings()?))
        }
        KeybindingsRequest::ResolvedBindings => {
            KeybindingsAnswer::Config(config(manager.get_resolved_bindings()?))
        }
        KeybindingsRequest::Reload => {
            manager.reload()?;
            KeybindingsAnswer::Done
        }
        KeybindingsRequest::EffectiveConfig => {
            KeybindingsAnswer::Config(config(manager.get_effective_config()?))
        }
    })
}

/// The wire form of keybinding keys.
pub(super) fn wire_keys(keys: &KeybindingKeys) -> host::KeybindingKeys {
    match keys {
        KeybindingKeys::Single(key) => host::KeybindingKeys::Single(key.clone()),
        KeybindingKeys::Multiple(keys) => host::KeybindingKeys::Multiple(keys.clone()),
    }
}

/// Keybinding keys from their wire form.
fn keys(keys: host::KeybindingKeys) -> KeybindingKeys {
    match keys {
        host::KeybindingKeys::Single(key) => KeybindingKeys::Single(key),
        host::KeybindingKeys::Multiple(keys) => KeybindingKeys::Multiple(keys),
    }
}

/// The wire form of bindings.
pub(super) fn wire_config(
    config: &KeybindingsConfig,
) -> Vec<(String, Option<host::KeybindingKeys>)> {
    config
        .iter()
        .map(|(id, keys)| (id.clone(), keys.as_ref().map(wire_keys)))
        .collect()
}

/// Bindings from their wire form.
fn config(config: Vec<(String, Option<host::KeybindingKeys>)>) -> KeybindingsConfig {
    config
        .into_iter()
        .map(|(id, bound)| (id, bound.map(keys)))
        .collect()
}

/// A definition from its wire form.
fn definition(definition: host::KeybindingDefinition) -> KeybindingDefinition {
    KeybindingDefinition {
        default_keys: keys(definition.default_keys),
        description: definition.description,
    }
}
