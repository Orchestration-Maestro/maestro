//! Application action defaults extending the terminal toolkit definitions.
#[cfg(not(target_arch = "wasm32"))]
mod reset;
#[cfg(not(target_arch = "wasm32"))]
pub use reset::{KeybindingsReset, reset_multiplexer_keybindings};

use maestro_tui::KeybindingsManager as TuiKeybindingsManager;
/// Toolkit key identifiers and ordered configuration records.
pub use maestro_tui::{KeyId, Keybinding, KeybindingsConfig};
use maestro_tui::{KeybindingDefinition, KeybindingDefinitions, KeybindingKeys, TUI_KEYBINDINGS};
use serde_json::{Map, Value};
use std::{collections::HashMap, ops::Deref, sync::LazyLock};

/// An open application action identifier.
pub type AppKeybinding = maestro_tui::Keybinding;
/// Ordered application defaults, retaining scalar and list shapes.
const APP_KEYBINDINGS: &[(&str, &[&str], bool, &str)] = &[
    ("app.interrupt", &["escape"], true, "Cancel or abort"),
    ("app.clear", &["ctrl+c"], true, "Clear editor"),
    ("app.exit", &["ctrl+d"], true, "Exit when editor is empty"),
    ("app.suspend", &["ctrl+z"], true, "Suspend to background"),
    (
        "app.thinking.cycle",
        &["shift+tab"],
        true,
        "Cycle thinking level",
    ),
    (
        "app.model.cycleForward",
        &["ctrl+p"],
        true,
        "Cycle to next model",
    ),
    (
        "app.model.cycleBackward",
        &["shift+ctrl+p"],
        true,
        "Cycle to previous model",
    ),
    ("app.model.select", &["ctrl+l"], true, "Open model selector"),
    ("app.tools.expand", &["ctrl+o"], true, "Toggle tool output"),
    (
        "app.thinking.toggle",
        &["ctrl+t"],
        true,
        "Toggle thinking blocks",
    ),
    (
        "app.session.toggleNamedFilter",
        &["ctrl+n"],
        true,
        "Toggle named session filter",
    ),
    (
        "app.editor.external",
        &["ctrl+g"],
        true,
        "Open external editor",
    ),
    (
        "app.message.followUp",
        &["alt+enter"],
        true,
        "Queue follow-up message",
    ),
    (
        "app.message.dequeue",
        &["alt+up"],
        true,
        "Restore queued messages",
    ),
    (
        "app.clipboard.pasteImage",
        &["ctrl+v"],
        true,
        "Paste image from clipboard",
    ),
    ("app.session.new", &[], false, "Start a new session"),
    ("app.session.tree", &[], false, "Open session tree"),
    ("app.session.fork", &[], false, "Fork current session"),
    ("app.session.resume", &[], false, "Resume a session"),
    (
        "app.tree.foldOrUp",
        &["ctrl+left", "alt+left"],
        false,
        "Fold tree branch or move up",
    ),
    (
        "app.tree.unfoldOrDown",
        &["ctrl+right", "alt+right"],
        false,
        "Unfold tree branch or move down",
    ),
    ("app.tree.editLabel", &["shift+l"], true, "Edit tree label"),
    (
        "app.tree.toggleLabelTimestamp",
        &["shift+t"],
        true,
        "Toggle tree label timestamps",
    ),
    (
        "app.session.togglePath",
        &["ctrl+p"],
        true,
        "Toggle session path display",
    ),
    (
        "app.session.toggleSort",
        &["ctrl+s"],
        true,
        "Toggle session sort mode",
    ),
    ("app.session.rename", &["ctrl+r"], true, "Rename session"),
    ("app.session.delete", &["ctrl+d"], true, "Delete session"),
    (
        "app.session.deleteNoninvasive",
        &["ctrl+backspace"],
        true,
        "Delete session when query is empty",
    ),
    ("app.models.save", &["ctrl+s"], true, "Save model selection"),
    (
        "app.models.enableAll",
        &["ctrl+a"],
        true,
        "Enable all models",
    ),
    ("app.models.clearAll", &["ctrl+x"], true, "Clear all models"),
    (
        "app.models.toggleProvider",
        &["ctrl+p"],
        true,
        "Toggle all models for provider",
    ),
    (
        "app.models.reorderUp",
        &["alt+up"],
        true,
        "Move model up in order",
    ),
    (
        "app.models.reorderDown",
        &["alt+down"],
        true,
        "Move model down in order",
    ),
    (
        "app.tree.filter.default",
        &["ctrl+d"],
        true,
        "Tree filter: default view",
    ),
    (
        "app.tree.filter.noTools",
        &["ctrl+t"],
        true,
        "Tree filter: hide tool results",
    ),
    (
        "app.tree.filter.userOnly",
        &["ctrl+u"],
        true,
        "Tree filter: user messages only",
    ),
    (
        "app.tree.filter.labeledOnly",
        &["ctrl+l"],
        true,
        "Tree filter: labeled entries only",
    ),
    (
        "app.tree.filter.all",
        &["ctrl+a"],
        true,
        "Tree filter: show all entries",
    ),
    (
        "app.tree.filter.cycleForward",
        &["ctrl+o"],
        true,
        "Tree filter: cycle forward",
    ),
    (
        "app.tree.filter.cycleBackward",
        &["shift+ctrl+o"],
        true,
        "Tree filter: cycle backward",
    ),
];
/// Toolkit definitions followed by the complete application action table.
pub static KEYBINDINGS: LazyLock<KeybindingDefinitions> =
    LazyLock::new(|| application_definitions(cfg!(windows)));
/// Builds platform-specific application defaults after the unchanged toolkit prefix.
fn application_definitions(windows: bool) -> KeybindingDefinitions {
    let mut definitions = TUI_KEYBINDINGS.clone();
    definitions.extend(
        APP_KEYBINDINGS
            .iter()
            .map(|&(id, keys, scalar, description)| {
                let default_keys = if windows && id == "app.suspend" {
                    KeybindingKeys::Multiple(Vec::new())
                } else if windows && id == "app.clipboard.pasteImage" {
                    KeybindingKeys::Single("alt+v".to_owned())
                } else if scalar {
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
            }),
    );
    definitions
}

/// Exact legacy action names and their current identifiers.
static KEYBINDING_NAME_MIGRATIONS: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    [
        ("cursorUp", "tui.editor.cursorUp"),
        ("cursorDown", "tui.editor.cursorDown"),
        ("cursorLeft", "tui.editor.cursorLeft"),
        ("cursorRight", "tui.editor.cursorRight"),
        ("cursorWordLeft", "tui.editor.cursorWordLeft"),
        ("cursorWordRight", "tui.editor.cursorWordRight"),
        ("cursorLineStart", "tui.editor.cursorLineStart"),
        ("cursorLineEnd", "tui.editor.cursorLineEnd"),
        ("jumpForward", "tui.editor.jumpForward"),
        ("jumpBackward", "tui.editor.jumpBackward"),
        ("pageUp", "tui.editor.pageUp"),
        ("pageDown", "tui.editor.pageDown"),
        ("deleteCharBackward", "tui.editor.deleteCharBackward"),
        ("deleteCharForward", "tui.editor.deleteCharForward"),
        ("deleteWordBackward", "tui.editor.deleteWordBackward"),
        ("deleteWordForward", "tui.editor.deleteWordForward"),
        ("deleteToLineStart", "tui.editor.deleteToLineStart"),
        ("deleteToLineEnd", "tui.editor.deleteToLineEnd"),
        ("yank", "tui.editor.yank"),
        ("yankPop", "tui.editor.yankPop"),
        ("undo", "tui.editor.undo"),
        ("newLine", "tui.input.newLine"),
        ("submit", "tui.input.submit"),
        ("tab", "tui.input.tab"),
        ("copy", "tui.input.copy"),
        ("selectUp", "tui.select.up"),
        ("selectDown", "tui.select.down"),
        ("selectPageUp", "tui.select.pageUp"),
        ("selectPageDown", "tui.select.pageDown"),
        ("selectConfirm", "tui.select.confirm"),
        ("selectCancel", "tui.select.cancel"),
        ("interrupt", "app.interrupt"),
        ("clear", "app.clear"),
        ("exit", "app.exit"),
        ("suspend", "app.suspend"),
        ("cycleThinkingLevel", "app.thinking.cycle"),
        ("cycleModelForward", "app.model.cycleForward"),
        ("cycleModelBackward", "app.model.cycleBackward"),
        ("selectModel", "app.model.select"),
        ("expandTools", "app.tools.expand"),
        ("toggleThinking", "app.thinking.toggle"),
        ("toggleSessionNamedFilter", "app.session.toggleNamedFilter"),
        ("externalEditor", "app.editor.external"),
        ("followUp", "app.message.followUp"),
        ("dequeue", "app.message.dequeue"),
        ("pasteImage", "app.clipboard.pasteImage"),
        ("newSession", "app.session.new"),
        ("tree", "app.session.tree"),
        ("fork", "app.session.fork"),
        ("resume", "app.session.resume"),
        ("treeFoldOrUp", "app.tree.foldOrUp"),
        ("treeUnfoldOrDown", "app.tree.unfoldOrDown"),
        ("treeEditLabel", "app.tree.editLabel"),
        ("treeToggleLabelTimestamp", "app.tree.toggleLabelTimestamp"),
        ("toggleSessionPath", "app.session.togglePath"),
        ("toggleSessionSort", "app.session.toggleSort"),
        ("renameSession", "app.session.rename"),
        ("deleteSession", "app.session.delete"),
        ("deleteSessionNoninvasive", "app.session.deleteNoninvasive"),
    ]
    .into_iter()
    .collect()
});
/// Converts legacy action names, preserving current-name precedence and arbitrary values.
///
/// Returns the canonically ordered object and whether any legacy name was encountered.
#[must_use]
pub fn migrate_keybindings_config(raw: &Map<String, Value>) -> (Map<String, Value>, bool) {
    let mut converted = Map::new();
    let mut migrated = false;
    for (key, value) in raw {
        let next = KEYBINDING_NAME_MIGRATIONS
            .get(key.as_str())
            .copied()
            .unwrap_or(key);
        if next != key {
            migrated = true;
            if raw.contains_key(next) {
                continue;
            }
        }
        converted.insert(next.to_owned(), value.clone());
    }
    (order_keybindings_config(converted), migrated)
}
/// Orders known actions before UTF-16-sorted extras, with canonical numeric keys first.
fn order_keybindings_config(mut config: Map<String, Value>) -> Map<String, Value> {
    let mut ordered = Map::new();
    for (id, _) in KEYBINDINGS.iter() {
        if let Some(value) = config.shift_remove(id) {
            ordered.insert(id.clone(), value);
        }
    }
    let mut extras: Vec<_> = config.into_iter().collect();
    extras.sort_by(|(left, _), (right, _)| maestro_text::utf16_order(left, right));
    ordered.extend(extras);
    maestro_json::canonical_entries(ordered)
        .into_iter()
        .collect()
}

/// Admits only scalar strings or all-string lists at the file boundary.
#[cfg(not(target_arch = "wasm32"))]
fn to_keybindings_config(config: Map<String, Value>) -> KeybindingsConfig {
    config
        .into_iter()
        .filter_map(|(id, value)| {
            let keys = match value {
                Value::String(key) => Some(KeybindingKeys::Single(key)),
                Value::Array(values) => values
                    .into_iter()
                    .map(|value| match value {
                        Value::String(key) => Some(key),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .map(KeybindingKeys::Multiple),
                _ => None,
            }?;
            Some((id, Some(keys)))
        })
        .collect()
}

/// Reads lossy UTF-8, admits an object, then converts names before typed admission.
#[cfg(not(target_arch = "wasm32"))]
fn load_from_file(path: &str) -> KeybindingsConfig {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(raw) =
        maestro_json::object::from_str::<Map<String, Value>>(&String::from_utf8_lossy(&bytes))
    else {
        return Vec::new();
    };
    let (config, _) = migrate_keybindings_config(&raw);
    to_keybindings_config(config)
}

/// Application defaults over one shared toolkit manager; cloned handles observe replacements.
#[derive(Clone, Debug)]
pub struct KeybindingsManager {
    /// Retained toolkit resolution owner.
    manager: TuiKeybindingsManager,
    /// Original file spelling, retained only on native targets.
    #[cfg(not(target_arch = "wasm32"))]
    config_path: Option<String>,
}
impl KeybindingsManager {
    /// Creates an inert manager with direct overrides; does not read or convert a file.
    #[must_use]
    pub fn new(user_bindings: KeybindingsConfig, config_path: Option<String>) -> Self {
        #[cfg(target_arch = "wasm32")]
        drop(config_path);
        Self {
            manager: TuiKeybindingsManager::new(KEYBINDINGS.clone(), user_bindings),
            #[cfg(not(target_arch = "wasm32"))]
            config_path,
        }
    }
    /// Loads overrides from the supplied root or the established application directory.
    ///
    /// # Errors
    /// Returns the configuration-root lookup error when the default root cannot be obtained.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn create(agent_dir: Option<&str>) -> std::io::Result<Self> {
        let agent_dir = match agent_dir {
            Some(path) => std::borrow::Cow::Borrowed(path),
            None => std::borrow::Cow::Owned(crate::configuration::config::get_agent_dir()?),
        };
        let path = maestro_path::join(&[&agent_dir, "keybindings.json"]);
        Ok(Self::new(load_from_file(&path), Some(path)))
    }
    /// Replaces overrides from the original path, or leaves pathless runtime state intact.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn reload(&self) {
        if let Some(path) = self.config_path.as_deref().filter(|path| !path.is_empty()) {
            self.manager.set_user_bindings(load_from_file(path));
        }
    }
    /// Returns an owned snapshot of resolved known actions.
    #[must_use]
    pub fn get_effective_config(&self) -> KeybindingsConfig {
        self.manager.get_resolved_bindings()
    }
}
impl Default for KeybindingsManager {
    fn default() -> Self {
        Self::new(Vec::new(), None)
    }
}
impl Deref for KeybindingsManager {
    type Target = TuiKeybindingsManager;
    fn deref(&self) -> &Self::Target {
        &self.manager
    }
}
#[cfg(test)]
mod tests;
