//! Supplied application keybindings through the extension capability.
#![cfg(test)]

use maestro_extensions::widgets::ExtensionKeybindings;
use maestro_tui::{
    KeybindingConflict, KeybindingDefinition, KeybindingKeys, KeybindingsConfig, KeybindingsManager,
};
use std::{cell::RefCell, rc::Rc};

/// One scalar binding in an owned configuration.
fn binding(action: &str, key: &str) -> (String, Option<KeybindingKeys>) {
    (action.into(), Some(KeybindingKeys::Single(key.into())))
}

#[test]
fn supplied_keybindings_keep_reload_and_owned_effective_config() {
    let definition = KeybindingDefinition {
        default_keys: KeybindingKeys::Single("f3".into()),
        description: Some("First action".into()),
    };
    let initial = vec![
        ("app.first".into(), None),
        ("app.second".into(), Some(KeybindingKeys::Multiple(vec![]))),
    ];
    let manager = KeybindingsManager::new(
        vec![
            ("app.first".into(), definition.clone()),
            ("app.second".into(), definition.clone()),
        ],
        initial.clone(),
    );
    let backing = Rc::new(RefCell::new(initial.clone()));
    let released = Rc::downgrade(&backing);
    let supplied = ExtensionKeybindings {
        manager: manager.clone(),
        reload: Some(Rc::new({
            let backing = backing.clone();
            move || manager.set_user_bindings(backing.borrow().clone())
        })),
    };
    let retained = supplied.clone();
    assert_eq!(supplied.get_user_bindings(), initial);
    assert_eq!(supplied.get_definition("app.first"), Some(definition));
    assert_eq!(
        supplied.get_effective_config(),
        vec![
            binding("app.first", "f3"),
            ("app.second".into(), Some(KeybindingKeys::Multiple(vec![]))),
        ]
    );
    let mut snapshot = supplied.get_effective_config();
    snapshot.clear();
    assert_eq!(supplied.get_keys("app.first"), ["f3"]);
    let replacement = vec![
        binding("unknown", "f8"),
        binding("app.second", "f9"),
        binding("app.first", "f9"),
    ];
    *backing.borrow_mut() = replacement.clone();
    drop(supplied);
    retained.reload();
    assert_eq!(retained.get_user_bindings(), replacement);
    assert_eq!(
        retained.get_effective_config(),
        vec![binding("app.first", "f9"), binding("app.second", "f9")]
    );
    assert_eq!(
        retained.get_effective_config(),
        retained.get_resolved_bindings()
    );
    assert_inherited_operations(&retained);
    drop(backing);
    assert!(released.upgrade().is_some());
    drop(retained);
    assert!(released.upgrade().is_none());
}

/// Inherited operations observe the same supplied manager before and after reload.
fn assert_inherited_operations(retained: &ExtensionKeybindings) {
    assert_eq!(retained.get_keys("app.first"), ["f9"]);
    assert!(retained.get_keys("unknown").is_empty());
    assert_eq!(
        retained.get_conflicts(),
        vec![KeybindingConflict {
            key: "f9".into(),
            keybindings: vec!["app.second".into(), "app.first".into()],
        }]
    );
    retained.set_user_bindings(vec![binding("app.first", "ctrl+c")]);
    assert!(retained.matches("\u{3}", "app.first"));
    retained.reload();
    assert!(!retained.matches("\u{3}", "app.first"));
}

#[test]
fn supplied_keybindings_without_reload_keep_runtime_overrides() {
    let supplied = ExtensionKeybindings {
        manager: KeybindingsManager::new(
            vec![(
                "app.first".into(),
                KeybindingDefinition {
                    default_keys: KeybindingKeys::Single("f3".into()),
                    description: None,
                },
            )],
            KeybindingsConfig::new(),
        ),
        reload: None,
    };
    let retained = supplied.clone();
    supplied.set_user_bindings(vec![binding("app.first", "f9")]);
    retained.reload();
    assert_eq!(
        supplied.get_effective_config(),
        vec![binding("app.first", "f9")]
    );
}
