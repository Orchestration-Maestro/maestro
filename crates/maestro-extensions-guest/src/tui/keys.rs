//! Keybindings managers, keybinding hints and editor styles the host keeps.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::widgets::host;
use super::{SelectListTheme, Style};
use crate::ExtensionResult;
use std::rc::Rc;

/// The keys of one keybinding: one key, or a list that may be empty or repeat a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeybindingKeys {
    /// One key.
    Single(String),
    /// Keys in their order.
    Multiple(Vec<String>),
}

/// One keybinding's default keys and description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingDefinition {
    /// Keys used while no user binding sets the keybinding.
    pub default_keys: KeybindingKeys,
    /// What the keybinding does.
    pub description: Option<String>,
}

/// Several keybindings a user bound to one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingConflict {
    /// The key.
    pub key: String,
    /// The keybindings that claim it, in claim order.
    pub keybindings: Vec<String>,
}

/// Keybindings in their order, each with its keys or unset.
pub type KeybindingsConfig = Vec<(String, Option<KeybindingKeys>)>;
/// Keybinding definitions in their order.
pub type KeybindingDefinitions = Vec<(String, KeybindingDefinition)>;

port! {
    /// Requests to a keybindings manager's owner; each reads its current bindings.
    KeybindingsPort for KeybindingsManager via port {
        /// Whether input `data` is one of the keys of `keybinding`.
        fn matches(data: &str, keybinding: &str) -> bool;
        /// The keys of `keybinding`, empty when it has none.
        fn get_keys(keybinding: &str) -> Vec<String>;
        /// The definition of `keybinding`, absent for an unknown one.
        fn get_definition(keybinding: &str) -> Option<KeybindingDefinition>;
        /// The keys several user bindings claim.
        fn get_conflicts() -> Vec<KeybindingConflict>;
        /// Replaces the user bindings.
        fn set_user_bindings(user_bindings: KeybindingsConfig) -> ();
        /// The user bindings as supplied.
        fn get_user_bindings() -> KeybindingsConfig;
        /// Each known keybinding with the keys it resolves to.
        fn get_resolved_bindings() -> KeybindingsConfig;
        /// Reloads the user bindings from the manager's file; a manager without one keeps them.
        fn reload() -> ();
        /// The bindings in effect, as the application reports them.
        fn get_effective_config() -> KeybindingsConfig;
    }
}

/// A keybindings manager the host keeps; clones share it and observe each change.
#[derive(Clone)]
pub struct KeybindingsManager {
    /// The owner.
    port: Rc<dyn KeybindingsPort>,
    /// The host resource behind the manager, which an editor can pass back.
    #[cfg(any(test, target_arch = "wasm32"))]
    host: Option<Rc<dyn std::any::Any>>,
}
impl KeybindingsManager {
    /// A manager of the application's keybindings with `user_bindings`, reloading from
    /// `config_path`.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(
        user_bindings: KeybindingsConfig,
        config_path: Option<&str>,
    ) -> ExtensionResult<Self> {
        host()?.keybindings(user_bindings, config_path)
    }
    /// A manager of the application's keybindings loaded from `keybindings.json` in
    /// `agent_dir`, or in the host's agent directory.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn create(agent_dir: Option<&str>) -> ExtensionResult<Self> {
        host()?.create_keybindings(agent_dir)
    }
    /// A manager of `definitions` with `user_bindings`.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn from_definitions(
        definitions: KeybindingDefinitions,
        user_bindings: KeybindingsConfig,
    ) -> ExtensionResult<Self> {
        host()?.keybindings_with_definitions(definitions, user_bindings)
    }
    /// A manager the host keeps as `owner`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn lent<P: KeybindingsPort + 'static>(owner: Rc<P>) -> Self {
        Self {
            port: owner.clone(),
            host: Some(owner),
        }
    }
    /// The host resource behind this manager, when the host keeps it as a `P`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn host<P: 'static>(&self) -> Option<Rc<P>> {
        self.host.clone()?.downcast().ok()
    }
}

/// The keybindings the host's terminal uses.
///
/// # Errors
/// Returns the host's failure.
pub fn get_keybindings() -> ExtensionResult<KeybindingsManager> {
    host()?.get_keybindings()
}

/// The keys of `keybinding` in the host's keybindings, joined with `/`.
///
/// # Errors
/// Returns the host's failure.
pub fn key_text(keybinding: &str) -> ExtensionResult<String> {
    host()?.key_text(keybinding)
}

/// The keys of `keybinding` and `description`, styled with the live theme.
///
/// # Errors
/// Returns the host's failure, such as a theme that is not published yet.
pub fn key_hint(keybinding: &str, description: &str) -> ExtensionResult<String> {
    host()?.key_hint(keybinding, description)
}

/// A literal `key` and `description`, styled with the live theme.
///
/// # Errors
/// Returns the host's failure, such as a theme that is not published yet.
pub fn raw_key_hint(key: &str, description: &str) -> ExtensionResult<String> {
    host()?.raw_key_hint(key, description)
}

/// Editor styles: the border, and the styles of its completion list.
#[derive(Clone)]
pub struct EditorTheme {
    /// The border.
    pub border_color: Style,
    /// The completion list.
    pub select_list: SelectListTheme,
}

/// The live theme's editor styles.
///
/// # Errors
/// Returns the host's failure.
pub fn get_editor_theme() -> ExtensionResult<EditorTheme> {
    host()?.editor_theme()
}
