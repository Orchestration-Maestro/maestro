//! Application action definitions extending the terminal toolkit defaults.
use maestro_tui::{KeybindingDefinition, KeybindingDefinitions, KeybindingKeys, TUI_KEYBINDINGS};
use std::sync::LazyLock;

/// Toolkit actions extended with application actions for caller-installed managers.
pub static KEYBINDINGS: LazyLock<KeybindingDefinitions> = LazyLock::new(|| {
    let mut definitions = TUI_KEYBINDINGS.clone();
    definitions.extend(
        [
            ("app.models.save", "ctrl+s", "Save model selection"),
            ("app.models.enableAll", "ctrl+a", "Enable all models"),
            ("app.models.clearAll", "ctrl+x", "Clear all models"),
            (
                "app.models.toggleProvider",
                "ctrl+p",
                "Toggle all models for provider",
            ),
            ("app.models.reorderUp", "alt+up", "Move model up in order"),
            (
                "app.models.reorderDown",
                "alt+down",
                "Move model down in order",
            ),
            ("app.editor.external", "ctrl+g", "Open external editor"),
            (
                "app.session.togglePath",
                "ctrl+p",
                "Toggle session path display",
            ),
            (
                "app.session.toggleSort",
                "ctrl+s",
                "Toggle session sort mode",
            ),
            (
                "app.session.toggleNamedFilter",
                "ctrl+n",
                "Toggle named session filter",
            ),
            (
                "app.tree.filter.default",
                "ctrl+d",
                "Tree filter: default view",
            ),
            (
                "app.tree.filter.noTools",
                "ctrl+t",
                "Tree filter: hide tool results",
            ),
            (
                "app.tree.filter.userOnly",
                "ctrl+u",
                "Tree filter: user messages only",
            ),
            (
                "app.tree.filter.labeledOnly",
                "ctrl+l",
                "Tree filter: labeled entries only",
            ),
            (
                "app.tree.filter.all",
                "ctrl+a",
                "Tree filter: show all entries",
            ),
            (
                "app.tree.filter.cycleForward",
                "ctrl+o",
                "Tree filter: cycle forward",
            ),
            (
                "app.tree.filter.cycleBackward",
                "shift+ctrl+o",
                "Tree filter: cycle backward",
            ),
        ]
        .into_iter()
        .map(|(id, key, description)| {
            (
                id.to_owned(),
                KeybindingDefinition {
                    default_keys: KeybindingKeys::Single(key.to_owned()),
                    description: Some(description.to_owned()),
                },
            )
        }),
    );
    for (id, keys, description) in [
        (
            "app.tree.foldOrUp",
            vec!["ctrl+left", "alt+left"],
            "Fold tree branch or move up",
        ),
        (
            "app.tree.unfoldOrDown",
            vec!["ctrl+right", "alt+right"],
            "Unfold tree branch or move down",
        ),
        ("app.tree.editLabel", vec!["shift+l"], "Edit tree label"),
        (
            "app.tree.toggleLabelTimestamp",
            vec!["shift+t"],
            "Toggle tree label timestamps",
        ),
    ] {
        definitions.push((
            id.to_owned(),
            KeybindingDefinition {
                default_keys: if keys.len() == 1 {
                    KeybindingKeys::Single(keys[0].to_owned())
                } else {
                    KeybindingKeys::Multiple(keys.into_iter().map(str::to_owned).collect())
                },
                description: Some(description.to_owned()),
            },
        ));
    }
    definitions
});
