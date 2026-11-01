//! Keybindings managers and keybinding hints, served on the local owner.
use crate::local_resources::{Lease, Message, Registry};
use crate::widget_reads::{Reply, WidgetMessage};
use maestro_extensions::widgets::{
    ExtensionKeybindings, ExtensionWidgets, KeybindingConflict, KeybindingDefinition,
    KeybindingKeys, KeybindingsConfig, get_keybindings, key_text,
};
use maestro_extensions::{KeybindingsManager, widgets};
use std::sync::Arc;
use wasmtime::component::{ComponentType, Lift, Lower};

/// The keys of one keybinding as they cross the boundary.
#[derive(Clone, ComponentType, Lift, Lower)]
#[component(variant)]
pub(crate) enum Keys {
    /// One key.
    #[component(name = "single")]
    Single(String),
    /// Keys in their order.
    #[component(name = "multiple")]
    Multiple(Vec<String>),
}

impl From<Keys> for KeybindingKeys {
    fn from(keys: Keys) -> Self {
        match keys {
            Keys::Single(key) => Self::Single(key),
            Keys::Multiple(keys) => Self::Multiple(keys),
        }
    }
}

impl From<KeybindingKeys> for Keys {
    fn from(keys: KeybindingKeys) -> Self {
        match keys {
            KeybindingKeys::Single(key) => Self::Single(key),
            KeybindingKeys::Multiple(keys) => Self::Multiple(keys),
        }
    }
}

/// One keybinding's definition as it crosses the boundary.
#[derive(Clone, ComponentType, Lift, Lower)]
#[component(record)]
pub(crate) struct Definition {
    /// Default keys.
    #[component(name = "default-keys")]
    default_keys: Keys,
    /// Description.
    description: Option<String>,
}

/// Several keybindings claiming one key, as they cross the boundary.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Conflict {
    /// The key.
    key: String,
    /// The claimants.
    keybindings: Vec<String>,
}

/// Bindings as they cross the boundary.
pub(crate) type Config = Vec<(String, Option<Keys>)>;

/// Bindings converted between their wire and toolkit forms.
pub(crate) fn config<A: Into<B>, B>(config: Vec<(String, Option<A>)>) -> Vec<(String, Option<B>)> {
    config
        .into_iter()
        .map(|(id, keys)| (id, keys.map(Into::into)))
        .collect()
}

/// Toolkit definitions from their wire form.
pub(crate) fn definitions(
    definitions: Vec<(String, Definition)>,
) -> widgets::KeybindingDefinitions {
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
    Definitions(widgets::KeybindingDefinitions, KeybindingsConfig),
    /// The host's terminal keybindings.
    Global,
}

/// One request on a manager.
pub(crate) enum KeysRequest {
    /// Whether input matches a keybinding.
    Matches(String, String, Reply<bool>),
    /// A keybinding's keys.
    Keys(String, Reply<Vec<String>>),
    /// A keybinding's definition.
    Definition(String, Reply<Option<Definition>>),
    /// The user binding conflicts.
    Conflicts(Reply<Vec<Conflict>>),
    /// Replaces the user bindings.
    SetUserBindings(KeybindingsConfig, Reply<()>),
    /// The user bindings.
    UserBindings(Reply<Config>),
    /// The resolved bindings.
    ResolvedBindings(Reply<Config>),
    /// Reloads the user bindings.
    Reload(Reply<()>),
}

impl KeysRequest {
    /// Returns a catchable failure.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        match self {
            Self::Matches(_, _, reply) => drop(reply.send(Err(error))),
            Self::Keys(_, reply) => drop(reply.send(Err(error))),
            Self::Definition(_, reply) => drop(reply.send(Err(error))),
            Self::Conflicts(reply) => drop(reply.send(Err(error))),
            Self::UserBindings(reply) | Self::ResolvedBindings(reply) => {
                drop(reply.send(Err(error)));
            }
            Self::SetUserBindings(_, reply) | Self::Reload(reply) => drop(reply.send(Err(error))),
        }
    }

    /// Answers the request from `keybindings`.
    pub(crate) fn answer(self, keybindings: &ExtensionKeybindings) {
        let manager = &keybindings.manager;
        match self {
            Self::Matches(data, id, reply) => drop(reply.send(Ok(manager.matches(&data, &id)))),
            Self::Keys(id, reply) => drop(reply.send(Ok(manager.get_keys(&id)))),
            Self::Definition(id, reply) => {
                let definition = manager.get_definition(&id).map(|definition| Definition {
                    default_keys: definition.default_keys.into(),
                    description: definition.description,
                });
                drop(reply.send(Ok(definition)));
            }
            Self::Conflicts(reply) => {
                let conflicts = manager.get_conflicts().into_iter().map(conflict).collect();
                drop(reply.send(Ok(conflicts)));
            }
            Self::SetUserBindings(user, reply) => {
                manager.set_user_bindings(user);
                drop(reply.send(Ok(())));
            }
            Self::UserBindings(reply) => drop(reply.send(Ok(config(manager.get_user_bindings())))),
            Self::ResolvedBindings(reply) => {
                drop(reply.send(Ok(config(manager.get_resolved_bindings()))));
            }
            Self::Reload(reply) => {
                if let Some(reload) = &keybindings.reload {
                    reload();
                }
                drop(reply.send(Ok(())));
            }
        }
    }
}

/// The wire form of a conflict.
fn conflict(conflict: KeybindingConflict) -> Conflict {
    Conflict {
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
pub(crate) struct KeysEndpoint(pub(crate) Arc<Lease>);

impl KeysEndpoint {
    /// Sends one request.
    pub(crate) fn read<T>(
        &self,
        request: impl FnOnce(Reply<T>) -> KeysRequest,
    ) -> Result<T, String> {
        self.0
            .request(|id, reply| Message::Widgets(WidgetMessage::Keys(id, request(reply))))
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
