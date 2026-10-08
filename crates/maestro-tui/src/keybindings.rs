//! Configurable terminal actions, literal key claims and a shared active manager.

use crate::{KeyId, matches_key};
use std::sync::{Arc, LazyLock, PoisonError, RwLock};

/// An open terminal action identifier.
pub type Keybinding = String;
/// The supplied scalar or list shape of a binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeybindingKeys {
    /// One supplied key.
    Single(KeyId),
    /// Zero or more supplied keys, including repetitions.
    Multiple(Vec<KeyId>),
}
/// Default keys and an optional action description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingDefinition {
    /// Keys used when an override is absent or unset.
    pub default_keys: KeybindingKeys,
    /// Human-readable action description.
    pub description: Option<String>,
}
/// Ordered action definitions; repeated IDs replace values in their first slot.
pub type KeybindingDefinitions = Vec<(Keybinding, KeybindingDefinition)>;
/// Ordered overrides; an unset value falls back to defaults.
pub type KeybindingsConfig = Vec<(Keybinding, Option<KeybindingKeys>)>;
/// Multiple known actions explicitly claiming one literal key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingConflict {
    /// The exact, case-sensitive claimed identifier.
    pub key: KeyId,
    /// Claimants in first-claim order.
    pub keybindings: Vec<Keybinding>,
}
/// The built-in terminal action definitions.
pub static TUI_KEYBINDINGS: LazyLock<KeybindingDefinitions> = LazyLock::new(|| {
    [
        ("tui.editor.cursorUp", &["up"][..], true, "Move cursor up"),
        (
            "tui.editor.cursorDown",
            &["down"][..],
            true,
            "Move cursor down",
        ),
        (
            "tui.editor.cursorLeft",
            &["left", "ctrl+b"][..],
            false,
            "Move cursor left",
        ),
        (
            "tui.editor.cursorRight",
            &["right", "ctrl+f"][..],
            false,
            "Move cursor right",
        ),
        (
            "tui.editor.cursorWordLeft",
            &["alt+left", "ctrl+left", "alt+b"][..],
            false,
            "Move cursor word left",
        ),
        (
            "tui.editor.cursorWordRight",
            &["alt+right", "ctrl+right", "alt+f"][..],
            false,
            "Move cursor word right",
        ),
        (
            "tui.editor.cursorLineStart",
            &["home", "ctrl+a"][..],
            false,
            "Move to line start",
        ),
        (
            "tui.editor.cursorLineEnd",
            &["end", "ctrl+e"][..],
            false,
            "Move to line end",
        ),
        (
            "tui.editor.jumpForward",
            &["ctrl+]"][..],
            true,
            "Jump forward to character",
        ),
        (
            "tui.editor.jumpBackward",
            &["ctrl+alt+]"][..],
            true,
            "Jump backward to character",
        ),
        ("tui.editor.pageUp", &["pageUp"][..], true, "Page up"),
        ("tui.editor.pageDown", &["pageDown"][..], true, "Page down"),
        (
            "tui.editor.deleteCharBackward",
            &["backspace"][..],
            true,
            "Delete character backward",
        ),
        (
            "tui.editor.deleteCharForward",
            &["delete", "ctrl+d"][..],
            false,
            "Delete character forward",
        ),
        (
            "tui.editor.deleteWordBackward",
            &["ctrl+w", "alt+backspace"][..],
            false,
            "Delete word backward",
        ),
        (
            "tui.editor.deleteWordForward",
            &["alt+d", "alt+delete"][..],
            false,
            "Delete word forward",
        ),
        (
            "tui.editor.deleteToLineStart",
            &["ctrl+u"][..],
            true,
            "Delete to line start",
        ),
        (
            "tui.editor.deleteToLineEnd",
            &["ctrl+k"][..],
            true,
            "Delete to line end",
        ),
        ("tui.editor.yank", &["ctrl+y"][..], true, "Yank"),
        ("tui.editor.yankPop", &["alt+y"][..], true, "Yank pop"),
        ("tui.editor.undo", &["ctrl+-"][..], true, "Undo"),
        (
            "tui.input.newLine",
            &["shift+enter"][..],
            true,
            "Insert newline",
        ),
        ("tui.input.submit", &["enter"][..], true, "Submit input"),
        ("tui.input.tab", &["tab"][..], true, "Tab / autocomplete"),
        ("tui.input.copy", &["ctrl+c"][..], true, "Copy selection"),
        ("tui.select.up", &["up"][..], true, "Move selection up"),
        (
            "tui.select.down",
            &["down"][..],
            true,
            "Move selection down",
        ),
        (
            "tui.select.pageUp",
            &["pageUp"][..],
            true,
            "Selection page up",
        ),
        (
            "tui.select.pageDown",
            &["pageDown"][..],
            true,
            "Selection page down",
        ),
        (
            "tui.select.confirm",
            &["enter"][..],
            true,
            "Confirm selection",
        ),
        (
            "tui.select.cancel",
            &["escape", "ctrl+c"][..],
            false,
            "Cancel selection",
        ),
    ]
    .into_iter()
    .map(|(id, keys, scalar, description)| {
        let default_keys = if scalar {
            KeybindingKeys::Single(keys[0].to_owned())
        } else {
            KeybindingKeys::Multiple(keys.iter().map(|key| (*key).to_owned()).collect())
        };
        (
            id.to_owned(),
            KeybindingDefinition {
                default_keys,
                description: Some(description.to_owned()),
            },
        )
    })
    .collect()
});

/// A shared manager handle; clones observe subsequent override replacements.
#[derive(Clone, Debug)]
pub struct KeybindingsManager {
    /// Immutable action definitions shared by all handles.
    definitions: Arc<KeybindingDefinitions>,
    /// Atomically published configuration and resolution.
    state: Arc<RwLock<ResolvedState>>,
}
/// One consistent view of supplied and normalized binding data.
#[derive(Debug)]
struct ResolvedState {
    /// Original override shapes.
    user: KeybindingsConfig,
    /// Normalized keys in definition order.
    keys: Vec<(Keybinding, Vec<KeyId>)>,
    /// Ordered direct-user conflicts.
    conflicts: Vec<KeybindingConflict>,
}
/// Replaces repeated record values without moving the first occurrence.
fn ordered_record<T>(entries: Vec<(Keybinding, T)>) -> Vec<(Keybinding, T)> {
    let mut result: Vec<(Keybinding, T)> = Vec::new();
    for (id, value) in entries {
        if let Some((_, previous)) = result.iter_mut().find(|(key, _)| key == &id) {
            *previous = value;
        } else {
            result.push((id, value));
        }
    }
    result
}
/// Deduplicates literal keys while preserving their first occurrence.
fn normalized(keys: &KeybindingKeys) -> Vec<KeyId> {
    let supplied = match keys {
        KeybindingKeys::Single(key) => std::slice::from_ref(key),
        KeybindingKeys::Multiple(keys) => keys.as_slice(),
    };
    let mut result = Vec::new();
    for key in supplied {
        if !result.contains(key) {
            result.push(key.clone());
        }
    }
    result
}
impl KeybindingsManager {
    /// Resolves only the supplied definitions with the supplied overrides.
    #[must_use]
    pub fn new(definitions: KeybindingDefinitions, user_bindings: KeybindingsConfig) -> Self {
        let definitions = Arc::new(ordered_record(definitions));
        let state = resolve(&definitions, ordered_record(user_bindings));
        Self {
            definitions,
            state: Arc::new(RwLock::new(state)),
        }
    }
    /// Returns an owned original definition, or no value for an unknown action.
    #[must_use]
    pub fn get_definition(&self, keybinding: &str) -> Option<KeybindingDefinition> {
        self.definitions
            .iter()
            .find(|(id, _)| id == keybinding)
            .map(|(_, definition)| definition.clone())
    }
}

/// Resolves overrides and accumulates known direct-user claims.
fn resolve(definitions: &KeybindingDefinitions, user: KeybindingsConfig) -> ResolvedState {
    let mut claims: Vec<KeybindingConflict> = Vec::new();
    for (id, supplied) in &user {
        if !definitions.iter().any(|(known, _)| known == id) {
            continue;
        }
        let Some(supplied) = supplied else { continue };
        for key in normalized(supplied) {
            if let Some(claim) = claims.iter_mut().find(|claim| claim.key == key) {
                claim.keybindings.push(id.clone());
            } else {
                claims.push(KeybindingConflict {
                    key,
                    keybindings: vec![id.clone()],
                });
            }
        }
    }
    let keys = definitions
        .iter()
        .map(|(id, definition)| {
            let override_keys = user
                .iter()
                .find(|(action, _)| action == id)
                .and_then(|(_, keys)| keys.as_ref());
            (
                id.clone(),
                normalized(override_keys.unwrap_or(&definition.default_keys)),
            )
        })
        .collect();
    let conflicts = claims
        .into_iter()
        .filter(|claim| claim.keybindings.len() > 1)
        .collect();
    ResolvedState {
        user,
        keys,
        conflicts,
    }
}
impl KeybindingsManager {
    /// Matches any resolved key using the current keyboard protocol mode.
    #[must_use]
    pub fn matches(&self, data: &str, keybinding: &str) -> bool {
        let state = self.state.read().unwrap_or_else(PoisonError::into_inner);
        state
            .keys
            .iter()
            .find(|(id, _)| id == keybinding)
            .is_some_and(|(_, keys)| keys.iter().any(|key| matches_key(data, key)))
    }
    /// Returns normalized keys, or an empty list for an unknown action.
    #[must_use]
    pub fn get_keys(&self, keybinding: &str) -> Vec<KeyId> {
        let state = self.state.read().unwrap_or_else(PoisonError::into_inner);
        state
            .keys
            .iter()
            .find(|(id, _)| id == keybinding)
            .map_or_else(Vec::new, |(_, keys)| keys.clone())
    }
    /// Returns owned conflicts in first-key-claim and first-claimant order.
    #[must_use]
    pub fn get_conflicts(&self) -> Vec<KeybindingConflict> {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .conflicts
            .clone()
    }
    /// Replaces all overrides, preparing resolution before publishing the new state.
    pub fn set_user_bindings(&self, user_bindings: KeybindingsConfig) {
        let replacement = resolve(&self.definitions, ordered_record(user_bindings));
        let previous = {
            let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
            std::mem::replace(&mut *state, replacement)
        };
        drop(previous);
    }
    /// Returns an owned snapshot retaining unknown actions, unset values and list shapes.
    #[must_use]
    pub fn get_user_bindings(&self) -> KeybindingsConfig {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .user
            .clone()
    }
    /// Returns known actions only, with normalized singletons represented as scalars.
    #[must_use]
    pub fn get_resolved_bindings(&self) -> KeybindingsConfig {
        let state = self.state.read().unwrap_or_else(PoisonError::into_inner);
        state
            .keys
            .iter()
            .map(|(id, keys)| {
                let value = match keys.as_slice() {
                    [key] => KeybindingKeys::Single(key.clone()),
                    keys => KeybindingKeys::Multiple(keys.to_vec()),
                };
                (id.clone(), Some(value))
            })
            .collect()
    }
}

/// Lazily initialized active manager, replaced without changing retained handles.
static GLOBAL_KEYBINDINGS: LazyLock<RwLock<KeybindingsManager>> =
    LazyLock::new(|| RwLock::new(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Vec::new())));

/// Returns a shared handle to the active manager, lazily supplying built-in defaults.
#[must_use]
pub fn get_keybindings() -> KeybindingsManager {
    GLOBAL_KEYBINDINGS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Replaces the active manager for future lookups; retained handles remain unchanged.
pub fn set_keybindings(keybindings: KeybindingsManager) {
    let previous = {
        let mut active = GLOBAL_KEYBINDINGS
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        std::mem::replace(&mut *active, keybindings)
    };
    drop(previous);
}
