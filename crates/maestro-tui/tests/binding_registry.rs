use maestro_tui::{
    KeybindingDefinition, KeybindingKeys, KeybindingsManager, TUI_KEYBINDINGS, get_keybindings,
    set_keybindings,
};

/// Restores the original manager after the isolated registry exercise.
struct RegistryRestore(KeybindingsManager);
impl Drop for RegistryRestore {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
#[test]
fn global_registry_is_lazy_shared_replaceable_and_restorable() {
    let initial = get_keybindings();
    let restore = RegistryRestore(initial.clone());
    assert_eq!(initial.get_resolved_bindings().len(), 31);
    initial.set_user_bindings(vec![(
        "tui.input.submit".into(),
        Some(KeybindingKeys::Single("ctrl+g".into())),
    )]);
    assert!(get_keybindings().matches("\x07", "tui.input.submit"));
    let mut definitions = TUI_KEYBINDINGS.clone();
    definitions.push((
        "plugin.run".into(),
        KeybindingDefinition {
            default_keys: KeybindingKeys::Single("enter".into()),
            description: None,
        },
    ));
    let supplied = KeybindingsManager::new(definitions, vec![]);
    set_keybindings(supplied.clone());
    let held = get_keybindings();
    supplied.set_user_bindings(vec![
        (
            "plugin.run".into(),
            Some(KeybindingKeys::Single("ctrl+x".into())),
        ),
        (
            "tui.input.submit".into(),
            Some(KeybindingKeys::Single("ctrl+x".into())),
        ),
    ]);
    assert!(get_keybindings().matches("\x18", "plugin.run"));
    assert!(held.matches("\x18", "plugin.run"));
    set_keybindings(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), vec![]));
    assert!(held.matches("\x18", "plugin.run"));
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/binding_cases.json")).unwrap();
    let expected = &corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "global")
        .unwrap()["expected"];
    let conflicts: Vec<_> = held
        .get_conflicts()
        .into_iter()
        .map(|c| serde_json::json!({"key": c.key, "keybindings": c.keybindings}))
        .collect();
    assert_eq!(serde_json::json!(conflicts), expected["heldConflicts"]);
    assert_eq!(
        serde_json::json!(get_keybindings().get_keys("plugin.run")),
        expected["newLacksCustom"]
    );
    assert_eq!(
        serde_json::json!(get_keybindings().get_keys("tui.input.submit")),
        expected["newDefaults"]
    );
    drop(restore);
    assert!(get_keybindings().matches("\x07", "tui.input.submit"));
}
