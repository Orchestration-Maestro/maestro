//! Keybindings managers and keybinding hints, served on the local owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::bindings::maestro::extension::host::{
    KeybindingConflict as WireConflict, KeybindingDefinition as WireDefinition,
    KeybindingKeys as WireKeys,
};
use crate::local_resources::{Lease, Registry, held::Keybindings};
use maestro_extensions::presentation::{
    ExtensionKeybindings, ExtensionWidgets, KeybindingConflict, KeybindingDefinition,
    KeybindingKeys, KeybindingsConfig, get_keybindings, key_text,
};
use maestro_extensions::{KeybindingsManager, presentation};
use std::sync::Arc;

impl From<WireKeys> for KeybindingKeys {
    fn from(keys: WireKeys) -> Self {
        match keys {
            WireKeys::Single(key) => Self::Single(key),
            WireKeys::Multiple(keys) => Self::Multiple(keys),
        }
    }
}

impl From<KeybindingKeys> for WireKeys {
    fn from(keys: KeybindingKeys) -> Self {
        match keys {
            KeybindingKeys::Single(key) => Self::Single(key),
            KeybindingKeys::Multiple(keys) => Self::Multiple(keys),
        }
    }
}

/// Bindings as they cross the boundary.
pub(crate) type Config = Vec<(String, Option<WireKeys>)>;

/// Bindings converted between their wire and toolkit forms.
pub(crate) fn config<A: Into<B>, B>(config: Vec<(String, Option<A>)>) -> Vec<(String, Option<B>)> {
    config
        .into_iter()
        .map(|(id, keys)| (id, keys.map(Into::into)))
        .collect()
}

/// Toolkit definitions from their wire form.
pub(crate) fn definitions(
    definitions: Vec<(String, WireDefinition)>,
) -> presentation::KeybindingDefinitions {
    definitions
        .into_iter()
        .map(|(id, definition)| {
            let definition = KeybindingDefinition {
                default_keys: definition.default_keys.into(),
                description: definition.description,
            };
            (id, definition)
        })
        .collect()
}

/// Where a new manager comes from.
pub(crate) enum KeysBuild {
    /// The application's definitions with user bindings and a file.
    Application(KeybindingsConfig, Option<String>),
    /// The application's definitions loaded from an agent directory.
    Created(Option<String>),
    /// Supplied definitions with user bindings.
    Definitions(presentation::KeybindingDefinitions, KeybindingsConfig),
    /// The host's terminal keybindings.
    Global,
}

/// The wire form of a conflict.
fn conflict(conflict: KeybindingConflict) -> WireConflict {
    WireConflict {
        key: conflict.key,
        keybindings: conflict.keybindings,
    }
}

/// Builds the manager `build` names.
pub(crate) fn build(
    build: KeysBuild,
    widgets: &dyn ExtensionWidgets,
) -> Result<ExtensionKeybindings, String> {
    Ok(match build {
        KeysBuild::Application(user, path) => widgets.keybindings(user, path),
        KeysBuild::Created(dir) => widgets.create_keybindings(dir)?,
        KeysBuild::Definitions(definitions, user) => ExtensionKeybindings {
            manager: KeybindingsManager::new(definitions, user),
            reload: None,
        },
        KeysBuild::Global => ExtensionKeybindings {
            manager: get_keybindings(),
            reload: None,
        },
    })
}

/// A keybinding hint request.
pub(crate) enum Hint {
    /// The keys of a keybinding.
    Text(String),
    /// A keybinding and a description.
    Keybinding(String, String),
    /// A literal key and a description.
    Raw(String, String),
}

/// Answers a hint from the host's keybindings and live theme.
pub(crate) fn hint(hint: Hint, widgets: &dyn ExtensionWidgets) -> Result<String, String> {
    match hint {
        Hint::Text(id) => Ok(key_text(&id)),
        Hint::Keybinding(id, description) => widgets.key_hint(&id, &description),
        Hint::Raw(key, description) => widgets.raw_key_hint(&key, &description),
    }
}

/// Store-safe identity of one keybindings manager.
#[derive(Clone)]
pub struct KeysEndpoint(pub(crate) Arc<Lease>);

impl KeysEndpoint {
    /// Whether input matches a keybinding.
    pub(crate) fn matches(&self, data: String, id: String) -> Result<bool, String> {
        self.0
            .run::<Keybindings, _>(move |keys, _| Ok(keys.manager.matches(&data, &id)))
    }

    /// A keybinding's keys.
    pub(crate) fn get_keys(&self, id: String) -> Result<Vec<String>, String> {
        self.0
            .run::<Keybindings, _>(move |keys, _| Ok(keys.manager.get_keys(&id)))
    }

    /// A keybinding's definition.
    pub(crate) fn get_definition(&self, id: String) -> Result<Option<WireDefinition>, String> {
        self.0.run::<Keybindings, _>(move |keys, _| {
            Ok(keys
                .manager
                .get_definition(&id)
                .map(|definition| WireDefinition {
                    default_keys: definition.default_keys.into(),
                    description: definition.description,
                }))
        })
    }

    /// The user binding conflicts.
    pub(crate) fn get_conflicts(&self) -> Result<Vec<WireConflict>, String> {
        self.0.run::<Keybindings, _>(|keys, _| {
            Ok(keys
                .manager
                .get_conflicts()
                .into_iter()
                .map(conflict)
                .collect())
        })
    }

    /// Replaces the user bindings.
    pub(crate) fn set_user_bindings(&self, user: KeybindingsConfig) -> Result<(), String> {
        self.0.run::<Keybindings, _>(move |keys, _| {
            keys.manager.set_user_bindings(user);
            Ok(())
        })
    }

    /// The user bindings.
    pub(crate) fn get_user_bindings(&self) -> Result<Config, String> {
        self.0
            .run::<Keybindings, _>(|keys, _| Ok(config(keys.manager.get_user_bindings())))
    }

    /// The resolved bindings.
    pub(crate) fn get_resolved_bindings(&self) -> Result<Config, String> {
        self.0
            .run::<Keybindings, _>(|keys, _| Ok(config(keys.manager.get_resolved_bindings())))
    }

    /// Reloads the user bindings from the manager's file, when it has one.
    pub(crate) fn reload(&self) -> Result<(), String> {
        self.0.run::<Keybindings, _>(|keys, _| {
            if let Some(reload) = &keys.reload {
                reload();
            }
            Ok(())
        })
    }
}

impl Registry {
    /// Keeps a keybindings manager.
    pub(crate) fn keybindings(
        &self,
        keybindings: ExtensionKeybindings,
    ) -> Result<KeysEndpoint, String> {
        self.keep_keybindings(keybindings).map(KeysEndpoint)
    }
}
