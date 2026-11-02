//! Keybindings managers both test hosts keep, with their literal requests.
//!
//! The host's keybindings define `tui.select.up` as `up` and `app.demo` as `ctrl+x` and `f1`.
//! A manager with a file reloads `app.demo` as `f2`. A hint is `<dim>keys<muted> description`.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::HashMap;

use serde_json::{Value, json};

use super::observed::Observed;
use crate::{
    KeybindingConflict, KeybindingDefinition, KeybindingDefinitions, KeybindingKeys,
    KeybindingsConfig,
};

/// One manager.
#[derive(Clone)]
pub struct Manager {
    /// Definitions in order.
    definitions: KeybindingDefinitions,
    /// User bindings as supplied.
    user: KeybindingsConfig,
    /// The file it reloads from.
    path: Option<String>,
}

/// Managers by key; key 0 is the host's terminal keybindings.
#[derive(Default)]
pub struct Managers(HashMap<u32, Manager>, u32);

/// The host's keybinding definitions.
pub fn defaults() -> KeybindingDefinitions {
    let definition = |keys: KeybindingKeys, description: &str| KeybindingDefinition {
        default_keys: keys,
        description: Some(description.to_owned()),
    };
    vec![
        (
            "tui.select.up".to_owned(),
            definition(KeybindingKeys::Single("up".to_owned()), "Up"),
        ),
        (
            "app.demo".to_owned(),
            definition(
                KeybindingKeys::Multiple(vec!["ctrl+x".to_owned(), "f1".to_owned()]),
                "Demo",
            ),
        ),
    ]
}

/// Keeps a new manager and returns its key.
pub fn keep(
    observed: &mut Observed,
    definitions: Option<KeybindingDefinitions>,
    user: KeybindingsConfig,
    path: Option<String>,
) -> u32 {
    let managers = &mut observed.ui.keys;
    managers.1 += 1;
    let key = managers.1;
    let definitions = definitions.unwrap_or_else(defaults);
    observed.log(format!("keys {key} new {}", json!([config(&user), path])));
    observed.ui.keys.0.insert(
        key,
        Manager {
            definitions,
            user,
            path,
        },
    );
    key
}

/// The manager under `key`; key 0 is the host's terminal keybindings.
fn manager(observed: &mut Observed, key: u32) -> &mut Manager {
    observed.ui.keys.0.entry(key).or_insert_with(|| Manager {
        definitions: defaults(),
        user: Vec::new(),
        path: None,
    })
}

/// Whether the manager under `key` binds `id` to the key `data`, without a record.
pub fn binds(observed: &mut Observed, key: u32, id: &str, data: &str) -> bool {
    manager(observed, key)
        .keys(id)
        .iter()
        .any(|bound| bound == data)
}

/// The keys of `keys`, without repetitions.
fn listed(keys: &KeybindingKeys) -> Vec<String> {
    let keys = match keys {
        KeybindingKeys::Single(key) => vec![key.clone()],
        KeybindingKeys::Multiple(keys) => keys.clone(),
    };
    let mut unique: Vec<String> = Vec::new();
    for key in keys {
        if !unique.contains(&key) {
            unique.push(key);
        }
    }
    unique
}

impl Manager {
    /// The keys `id` resolves to.
    fn keys(&self, id: &str) -> Vec<String> {
        let user = self.user.iter().rev().find(|(bound, _)| bound == id);
        match user {
            Some((_, Some(keys))) => listed(keys),
            _ => self
                .definitions
                .iter()
                .find(|(defined, _)| defined == id)
                .map(|(_, definition)| listed(&definition.default_keys))
                .unwrap_or_default(),
        }
    }

    /// Each definition with its resolved keys.
    fn resolved(&self) -> KeybindingsConfig {
        self.definitions
            .iter()
            .map(|(id, _)| (id.clone(), Some(KeybindingKeys::Multiple(self.keys(id)))))
            .collect()
    }

    /// The keys several known user bindings claim.
    fn conflicts(&self) -> Vec<KeybindingConflict> {
        let mut claims: Vec<KeybindingConflict> = Vec::new();
        let known = |id: &str| self.definitions.iter().any(|(defined, _)| defined == id);
        for (id, keys) in self.user.iter().filter(|(id, _)| known(id)) {
            for key in keys.as_ref().map(listed).unwrap_or_default() {
                match claims.iter_mut().find(|claim| claim.key == key) {
                    Some(claim) => claim.keybindings.push(id.clone()),
                    None => claims.push(KeybindingConflict {
                        key,
                        keybindings: vec![id.clone()],
                    }),
                }
            }
        }
        claims.retain(|claim| claim.keybindings.len() > 1);
        claims
    }
}

/// One request on a manager.
pub enum Request {
    /// Whether input matches a keybinding.
    Matches(String, String),
    /// A keybinding's keys.
    Keys(String),
    /// A keybinding's definition.
    Definition(String),
    /// The user binding conflicts.
    Conflicts,
    /// Replaces the user bindings.
    SetUserBindings(KeybindingsConfig),
    /// The user bindings.
    UserBindings,
    /// The resolved bindings.
    ResolvedBindings,
    /// Reloads the user bindings.
    Reload,
    /// The effective bindings.
    EffectiveConfig,
}

/// What a request answers.
pub enum Answer {
    /// Nothing.
    Done,
    /// Whether input matched.
    Matched(bool),
    /// Keys.
    Keys(Vec<String>),
    /// A definition.
    Definition(Option<KeybindingDefinition>),
    /// Conflicts.
    Conflicts(Vec<KeybindingConflict>),
    /// Bindings.
    Config(KeybindingsConfig),
}

/// Records and answers one request on the manager under `key`.
pub fn request(observed: &mut Observed, key: u32, request: Request) -> Answer {
    let name = match &request {
        Request::Matches(data, id) => format!("matches {}", json!([data, id])),
        Request::Keys(id) => format!("getKeys {}", json!(id)),
        Request::Definition(id) => format!("getDefinition {}", json!(id)),
        Request::Conflicts => "getConflicts".to_owned(),
        Request::SetUserBindings(user) => format!("setUserBindings {}", config(user)),
        Request::UserBindings => "getUserBindings".to_owned(),
        Request::ResolvedBindings => "getResolvedBindings".to_owned(),
        Request::Reload => "reload".to_owned(),
        Request::EffectiveConfig => "getEffectiveConfig".to_owned(),
    };
    observed.log(format!("keys {key} {name}"));
    let manager = manager(observed, key);
    match request {
        Request::Matches(data, id) => Answer::Matched(manager.keys(&id).contains(&data)),
        Request::Keys(id) => Answer::Keys(manager.keys(&id)),
        Request::Definition(id) => Answer::Definition(
            manager
                .definitions
                .iter()
                .find(|(defined, _)| *defined == id)
                .map(|(_, definition)| definition.clone()),
        ),
        Request::Conflicts => Answer::Conflicts(manager.conflicts()),
        Request::SetUserBindings(user) => {
            manager.user = user;
            Answer::Done
        }
        Request::UserBindings => Answer::Config(manager.user.clone()),
        Request::ResolvedBindings | Request::EffectiveConfig => Answer::Config(manager.resolved()),
        Request::Reload => {
            if manager.path.is_some() {
                manager.user = vec![(
                    "app.demo".to_owned(),
                    Some(KeybindingKeys::Single("f2".to_owned())),
                )];
            }
            Answer::Done
        }
    }
}

/// A keybinding's keys joined with `/`, or a styled hint, recorded.
pub fn hint(
    observed: &mut Observed,
    keybinding: Option<&str>,
    key: Option<&str>,
    description: Option<&str>,
) -> String {
    observed.log(format!("hint {}", json!([keybinding, key, description])));
    let text = match (keybinding, key) {
        (Some(keybinding), _) => manager(observed, 0).keys(keybinding).join("/"),
        (None, key) => key.unwrap_or_default().to_owned(),
    };
    match description {
        Some(description) => format!("<dim>{text}<muted> {description}"),
        None => text,
    }
}

/// The JSON form of bindings.
pub fn config(config: &KeybindingsConfig) -> Value {
    json!(
        config
            .iter()
            .map(|(id, keys)| json!([id, keys.as_ref().map(keys_json)]))
            .collect::<Vec<_>>()
    )
}

/// The JSON form of keys.
pub fn keys_json(keys: &KeybindingKeys) -> Value {
    match keys {
        KeybindingKeys::Single(key) => json!(key),
        KeybindingKeys::Multiple(keys) => json!(keys),
    }
}
