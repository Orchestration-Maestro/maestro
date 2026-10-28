//! Configurable application actions through the public application boundary.
#![cfg(test)]
#![cfg(unix)]
use crate::child;
use maestro_app::presentation_data::keybindings::migrate_keybindings_config;
use maestro_app::presentation_data::keybindings::{KeyId, Keybinding, KeybindingsConfig};
use maestro_app::{AppKeybinding, KeybindingsManager};
use maestro_tui::KeybindingKeys;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case<I, E> {
    input: I,
    expected: E,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInput {
    raw: Map<String, Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conversion {
    config: Map<String, Value>,
    order: Vec<String>,
    migrated: bool,
}
fn rows<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<Case<I, E>> {
    let mut corpus: Map<String, Value> =
        serde_json::from_str(include_str!("fixtures/application_keys.json")).unwrap();
    assert_eq!(corpus.len(), 21);
    assert_eq!(
        corpus
            .values()
            .map(|group| group.as_array().unwrap().len())
            .sum::<usize>(),
        56
    );
    for group in corpus.values() {
        let cases = group.as_array().unwrap();
        let unique: std::collections::HashSet<_> = cases
            .iter()
            .map(|case| serde_json::to_string(&case["input"]).unwrap())
            .collect();
        assert_eq!(unique.len(), cases.len());
    }
    serde_json::from_value(corpus.shift_remove(name).unwrap()).unwrap()
}
fn convert(name: &str) {
    for case in rows::<RawInput, Conversion>(name) {
        let (config, migrated) = migrate_keybindings_config(&case.input.raw);
        assert_eq!(
            config.keys().collect::<Vec<_>>(),
            case.expected.order.iter().collect::<Vec<_>>()
        );
        assert_eq!(config, case.expected.config);
        assert_eq!(migrated, case.expected.migrated);
    }
}
#[test]
fn legacy_action_names_convert_without_changing_binding_values() {
    convert("legacy_action_names_convert_without_changing_binding_values");
}

#[test]
fn current_action_names_win_in_either_input_order() {
    convert("current_action_names_win_in_either_input_order");
}

#[test]
fn current_action_names_win_even_when_the_current_value_is_unusable() {
    convert("current_action_names_win_even_when_the_current_value_is_unusable");
}

#[test]
fn converted_configuration_uses_definition_order_and_utf16_extra_order() {
    convert("converted_configuration_uses_definition_order_and_utf16_extra_order");
}

#[test]
fn prototype_named_actions_remain_literal_unknown_entries() {
    convert("prototype_named_actions_remain_literal_unknown_entries");
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonInput {
    json: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeConversion {
    config: Map<String, Value>,
    order: Vec<String>,
    migrated: bool,
    negative_zero: bool,
    pretty: String,
}
#[test]
fn conversion_preserves_arbitrary_unknown_trees_and_signed_zero() {
    for case in rows::<JsonInput, TreeConversion>(
        "conversion_preserves_arbitrary_unknown_trees_and_signed_zero",
    ) {
        let raw = serde_json::from_str(&case.input.json).unwrap();
        let (config, migrated) = migrate_keybindings_config(&raw);
        assert_eq!(
            maestro_json::pretty_object(&config).unwrap(),
            maestro_json::pretty_object(&case.expected.config).unwrap()
        );
        assert_eq!(
            config.keys().collect::<Vec<_>>(),
            case.expected.order.iter().collect::<Vec<_>>()
        );
        assert_eq!(migrated, case.expected.migrated);
        assert_eq!(
            config["unknownNumber"].as_f64().unwrap().is_sign_negative(),
            case.expected.negative_zero
        );
        assert_eq!(
            maestro_json::pretty_object(&config).unwrap(),
            case.expected.pretty
        );
    }
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    user: Map<String, Value>,
    user_order: Vec<String>,
    effective: Map<String, Value>,
    effective_order: Vec<String>,
    conflicts: Vec<Conflict>,
}
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Conflict {
    key: String,
    keybindings: Vec<String>,
}
fn binding_value(keys: Option<KeybindingKeys>) -> Value {
    match keys {
        None => Value::Null,
        Some(KeybindingKeys::Single(key)) => serde_json::json!(key),
        Some(KeybindingKeys::Multiple(keys)) => serde_json::json!(keys),
    }
}
fn config_value(config: KeybindingsConfig) -> Map<String, Value> {
    config
        .into_iter()
        .map(|(id, keys)| (id, binding_value(keys)))
        .collect()
}
fn typed_bindings(bindings: Map<String, Value>) -> KeybindingsConfig {
    bindings
        .into_iter()
        .map(|(id, value)| {
            let keys = match value {
                Value::String(key) => KeybindingKeys::Single(key),
                Value::Array(keys) => KeybindingKeys::Multiple(
                    keys.into_iter()
                        .map(|key| key.as_str().unwrap().to_owned())
                        .collect(),
                ),
                _ => panic!("invalid fixture binding"),
            };
            (id, Some(keys))
        })
        .collect()
}
fn snapshot(manager: &KeybindingsManager) -> Snapshot {
    let user = config_value(manager.get_user_bindings());
    let effective = config_value(manager.get_effective_config());
    Snapshot {
        user_order: user.keys().cloned().collect(),
        effective_order: effective.keys().cloned().collect(),
        user,
        effective,
        conflicts: manager
            .get_conflicts()
            .into_iter()
            .map(|conflict| Conflict {
                key: conflict.key,
                keybindings: conflict.keybindings,
            })
            .collect(),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructorInput {
    bindings: Map<String, Value>,
    config_path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructorExpected {
    user: Map<String, Value>,
    user_order: Vec<String>,
    effective: Map<String, Value>,
    effective_order: Vec<String>,
    conflicts: Vec<Conflict>,
    file_text: String,
}
#[test]
fn constructor_is_inert_and_keeps_direct_legacy_action_ids() {
    for case in rows::<ConstructorInput, ConstructorExpected>(
        "constructor_is_inert_and_keeps_direct_legacy_action_ids",
    ) {
        let dir = child::Scratch::new();
        let path = dir.join(&case.input.config_path);
        std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        std::fs::write(&path, &case.expected.file_text).unwrap();
        let manager =
            KeybindingsManager::new(typed_bindings(case.input.bindings), Some(path.clone()));
        let expected = case.expected;
        assert_eq!(
            snapshot(&manager),
            Snapshot {
                user: expected.user,
                user_order: expected.user_order,
                effective: expected.effective,
                effective_order: expected.effective_order,
                conflicts: expected.conflicts
            }
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected.file_text);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextInput {
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileExpected {
    user: Map<String, Value>,
    user_order: Vec<String>,
    effective: Map<String, Value>,
    effective_order: Vec<String>,
    conflicts: Vec<Conflict>,
    before: String,
    after: String,
}
#[test]
fn file_loading_converts_legacy_names_without_rewriting_the_file() {
    for case in rows::<TextInput, FileExpected>(
        "file_loading_converts_legacy_names_without_rewriting_the_file",
    ) {
        let dir = child::Scratch::new();
        let path = dir.join("keybindings.json");
        std::fs::write(&path, &case.input.text).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            case.expected.before
        );
        let manager = KeybindingsManager::create(Some(dir.0.path().to_str().unwrap())).unwrap();
        let expected = case.expected;
        assert_eq!(
            snapshot(&manager),
            Snapshot {
                user: expected.user,
                user_order: expected.user_order,
                effective: expected.effective,
                effective_order: expected.effective_order,
                conflicts: expected.conflicts
            }
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected.after);
    }
}

fn load_text(name: &str) {
    for case in rows::<TextInput, Snapshot>(name) {
        let dir = child::Scratch::new();
        std::fs::write(dir.join("keybindings.json"), case.input.text).unwrap();
        let manager = KeybindingsManager::create(Some(dir.0.path().to_str().unwrap())).unwrap();
        assert_eq!(snapshot(&manager), case.expected);
    }
}

#[test]
fn file_loading_accepts_only_objects_with_string_or_string_list_bindings() {
    load_text("file_loading_accepts_only_objects_with_string_or_string_list_bindings");
}

#[test]
fn native_json_read_failures_use_the_unreadable_file_fallback() {
    load_text("native_json_read_failures_use_the_unreadable_file_fallback");
}

#[test]
fn file_loading_keeps_prototype_named_unknown_actions() {
    load_text("file_loading_keeps_prototype_named_unknown_actions");
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
enum ReadInput {
    InvalidUtf8 { bytes: Vec<u8> },
    Directory {},
    Missing {},
}
#[test]
fn file_loading_uses_lossy_utf8_and_ignores_read_failures() {
    for case in
        rows::<ReadInput, Snapshot>("file_loading_uses_lossy_utf8_and_ignores_read_failures")
    {
        let dir = child::Scratch::new();
        let path = dir.join("keybindings.json");
        match case.input {
            ReadInput::InvalidUtf8 { bytes } => std::fs::write(&path, bytes).unwrap(),
            ReadInput::Directory {} => std::fs::create_dir(&path).unwrap(),
            ReadInput::Missing {} => {}
        }
        assert_eq!(
            snapshot(&KeybindingsManager::create(Some(dir.0.path().to_str().unwrap())).unwrap()),
            case.expected
        );
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkInput {
    dangling: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkExpected {
    user: Map<String, Value>,
    user_order: Vec<String>,
    effective: Map<String, Value>,
    effective_order: Vec<String>,
    conflicts: Vec<Conflict>,
    link_stays_symlink: bool,
    target_text: Option<String>,
}
#[test]
fn file_loading_follows_native_symlinks_without_rewriting_the_target() {
    for case in rows::<LinkInput, LinkExpected>(
        "file_loading_follows_native_symlinks_without_rewriting_the_target",
    ) {
        let dir = child::Scratch::new();
        let target = dir.join("target.json");
        if !case.input.dangling {
            std::fs::write(&target, "{\"app.clear\":\"alt+c\"}").unwrap();
        }
        let path = dir.join("keybindings.json");
        std::os::unix::fs::symlink("target.json", &path).unwrap();
        let manager = KeybindingsManager::create(Some(dir.0.path().to_str().unwrap())).unwrap();
        let expected = case.expected;
        assert_eq!(
            snapshot(&manager),
            Snapshot {
                user: expected.user,
                user_order: expected.user_order,
                effective: expected.effective,
                effective_order: expected.effective_order,
                conflicts: expected.conflicts
            }
        );
        assert_eq!(
            std::fs::symlink_metadata(path).unwrap().is_symlink(),
            expected.link_stays_symlink
        );
        assert_eq!(std::fs::read_to_string(target).ok(), expected.target_text);
    }
}

fn isolated(name: &str, run: impl FnOnce(&std::path::Path)) {
    if maestro_test_process::selection().is_child()
        && maestro_test_process::selection().runs_case(name)
    {
        run(&std::env::current_dir().unwrap());
    } else {
        let dir = child::Scratch::new();
        child::probe(
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .case(name),
            dir.0.path(),
            &[("MAESTRO_CODING_AGENT_DIR", "decoy")],
            &[],
        );
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReloadInput {
    relative: bool,
    initial: Map<String, Value>,
    target: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeforeAfter {
    before: Snapshot,
    after: Snapshot,
}
#[test]
fn constructor_reload_reads_only_the_supplied_config_path() {
    isolated(
        "constructor_reload_reads_only_the_supplied_config_path",
        |root| {
            for case in rows::<ReloadInput, BeforeAfter>(
                "constructor_reload_reads_only_the_supplied_config_path",
            ) {
                std::fs::write(root.join("keybindings.json"), "{\"app.clear\":\"alt+q\"}").unwrap();
                std::fs::write(root.join(&case.input.target), "{\"app.clear\":\"alt+r\"}").unwrap();
                let path = if case.input.relative {
                    format!("./{}", case.input.target)
                } else {
                    root.join(&case.input.target).to_str().unwrap().to_owned()
                };
                let manager =
                    KeybindingsManager::new(typed_bindings(case.input.initial), Some(path));
                assert_eq!(snapshot(&manager), case.expected.before);
                manager.reload();
                assert_eq!(snapshot(&manager), case.expected.after);
            }
        },
    );
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum RootSelection {
    Default,
    Supplied,
    Empty,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootInput {
    root_selection: RootSelection,
}
#[test]
fn file_factory_prefers_the_supplied_root_to_the_environment() {
    isolated(
        "file_factory_prefers_the_supplied_root_to_the_environment",
        |root| {
            std::fs::create_dir("decoy").unwrap();
            std::fs::write("decoy/keybindings.json", "{\"app.clear\":\"alt+b\"}").unwrap();
            std::fs::write("keybindings.json", "{\"app.clear\":\"alt+a\"}").unwrap();
            for case in rows::<RootInput, Snapshot>(
                "file_factory_prefers_the_supplied_root_to_the_environment",
            ) {
                let path = match case.input.root_selection {
                    RootSelection::Default => None,
                    RootSelection::Supplied => Some(root.to_str().unwrap()),
                    RootSelection::Empty => Some(""),
                };
                let manager = KeybindingsManager::create(path).unwrap();
                assert_eq!(snapshot(&manager), case.expected);
            }
        },
    );
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveInput {
    initial: String,
    replacement: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveExpected {
    before: Snapshot,
    after: Snapshot,
    retained_keys: Vec<String>,
    installed_keys: Vec<String>,
}
#[test]
fn reload_uses_the_original_path_and_updates_retained_toolkit_handles() {
    isolated(
        "reload_uses_the_original_path_and_updates_retained_toolkit_handles",
        |root| {
            for case in rows::<LiveInput, LiveExpected>(
                "reload_uses_the_original_path_and_updates_retained_toolkit_handles",
            ) {
                std::fs::create_dir("decoy").unwrap();
                std::fs::write("decoy/keybindings.json", "{\"app.clear\":\"alt+b\"}").unwrap();
                std::fs::write("keybindings.json", case.input.initial).unwrap();
                let manager = KeybindingsManager::create(Some(root.to_str().unwrap())).unwrap();
                let retained = manager.clone();
                maestro_tui::set_keybindings((*manager).clone());
                let installed = maestro_tui::get_keybindings();
                assert_eq!(snapshot(&manager), case.expected.before);
                std::fs::write("keybindings.json", case.input.replacement).unwrap();
                manager.reload();
                assert_eq!(snapshot(&manager), case.expected.after);
                assert_eq!(retained.get_keys("app.clear"), case.expected.retained_keys);
                assert_eq!(
                    installed.get_keys("app.clear"),
                    case.expected.installed_keys
                );
            }
        },
    );
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplacementInput {
    replacement: Option<String>,
}
#[test]
fn reload_missing_or_invalid_data_replaces_user_overrides_with_defaults() {
    for case in rows::<ReplacementInput, Snapshot>(
        "reload_missing_or_invalid_data_replaces_user_overrides_with_defaults",
    ) {
        let dir = child::Scratch::new();
        let path = dir.join("keybindings.json");
        std::fs::write(
            &path,
            "{\"app.clear\":\"alt+a\",\"custom.action\":\"ctrl+q\"}",
        )
        .unwrap();
        let manager = KeybindingsManager::create(Some(dir.0.path().to_str().unwrap())).unwrap();
        assert_eq!(manager.get_keys("app.clear"), ["alt+a"]);
        assert_eq!(manager.get_user_bindings().len(), 2);
        match case.input.replacement {
            Some(text) => std::fs::write(&path, text).unwrap(),
            None => std::fs::remove_file(&path).unwrap(),
        }
        manager.reload();
        assert_eq!(snapshot(&manager), case.expected);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathlessInput {
    config_path: Option<String>,
    initial: Map<String, Value>,
    replacement: Map<String, Value>,
}
#[test]
fn pathless_reload_preserves_runtime_bindings() {
    for case in rows::<PathlessInput, BeforeAfter>("pathless_reload_preserves_runtime_bindings") {
        let manager =
            KeybindingsManager::new(typed_bindings(case.input.initial), case.input.config_path);
        manager.set_user_bindings(typed_bindings(case.input.replacement));
        assert_eq!(snapshot(&manager), case.expected.before);
        manager.reload();
        assert_eq!(snapshot(&manager), case.expected.after);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingsInput {
    bindings: Map<String, Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstalledExpected {
    before: Snapshot,
    after: Snapshot,
    active_keys: Vec<String>,
    confirm_matches: bool,
    old_submit_does_not_confirm: bool,
}
#[test]
fn installed_toolkit_manager_keeps_reserved_keys_and_live_application_bindings() {
    isolated(
        "installed_toolkit_manager_keeps_reserved_keys_and_live_application_bindings",
        |_| {
            for case in rows::<BindingsInput, InstalledExpected>(
                "installed_toolkit_manager_keeps_reserved_keys_and_live_application_bindings",
            ) {
                let manager = KeybindingsManager::new(typed_bindings(case.input.bindings), None);
                maestro_tui::set_keybindings((*manager).clone());
                let active = maestro_tui::get_keybindings();
                assert_eq!(snapshot(&manager), case.expected.before);
                assert_eq!(active.get_keys("tui.select.confirm"), ["enter"]);
                assert!(!active.matches("\x18", "tui.select.confirm"));
                manager.set_user_bindings(typed_bindings(
                    serde_json::from_value(serde_json::json!({"app.interrupt": "alt+x"})).unwrap(),
                ));
                assert_eq!(snapshot(&manager), case.expected.after);
                assert_eq!(active.get_keys("app.interrupt"), case.expected.active_keys);
                assert_eq!(
                    active.matches("\r", "tui.select.confirm"),
                    case.expected.confirm_matches
                );
                assert_eq!(
                    active.matches("\x18", "tui.select.confirm"),
                    case.expected.old_submit_does_not_confirm
                );
            }
        },
    );
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DefinitionExpected {
    default_keys: Value,
    description: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectiveExpected {
    before: Snapshot,
    after: Snapshot,
    clear_definition: DefinitionExpected,
    unknown_definition_absent: bool,
}
#[test]
fn effective_snapshots_are_owned_and_inherited_operations_stay_available() {
    for case in rows::<BindingsInput, EffectiveExpected>(
        "effective_snapshots_are_owned_and_inherited_operations_stay_available",
    ) {
        let manager = KeybindingsManager::new(typed_bindings(case.input.bindings), None);
        let _: KeybindingsConfig = manager.get_effective_config();
        let action: AppKeybinding = "app.clear".into();
        let borrowed_action: Keybinding = action;
        let keys: Vec<KeyId> = manager.get_keys(&borrowed_action);
        assert_eq!(keys, ["ctrl+x"]);
        assert_eq!(snapshot(&manager), case.expected.before);
        assert_eq!(
            manager.get_effective_config(),
            manager.get_resolved_bindings()
        );
        manager.set_user_bindings(typed_bindings(serde_json::from_value(serde_json::json!({"app.interrupt":"ctrl+x", "app.clear":"ctrl+x", "future.action":"alt+f"})).unwrap()));
        assert_eq!(snapshot(&manager), case.expected.after);
        let definition = manager.get_definition("app.clear").unwrap();
        assert_eq!(
            binding_value(Some(definition.default_keys)),
            case.expected.clear_definition.default_keys
        );
        assert_eq!(
            definition.description.as_deref(),
            Some(case.expected.clear_definition.description.as_str())
        );
        assert_eq!(
            manager.get_definition("future.action").is_none(),
            case.expected.unknown_definition_absent
        );
        assert!(manager.get_keys("future.action").is_empty());
        assert!(!manager.matches("\x18", "future.action"));
    }
}

#[test]
fn reset_keys_backs_up_exact_bytes_and_preserves_other_actions() {
    use maestro_app::presentation_data::keybindings::reset_multiplexer_keybindings;
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("keybindings.json");
    let original = b"{\n  \"app.exit\": \"ctrl+d\",\n  \"unknown\": {\"kept\":true},\n  \"multiplexer.focus\": [\"alt+h\"],\n  \"multiplexer.\": null,\n  \"multiplexer\": 42\n}\n";
    std::fs::write(&path, original).unwrap();
    let reset =
        reset_multiplexer_keybindings(scratch.path().to_str().unwrap(), 1_700_000_000).unwrap();
    assert_eq!(
        reset.backup_path,
        scratch.path().join("keybindings.json.bak-1700000000")
    );
    assert_eq!(std::fs::read(&reset.backup_path).unwrap(), original);
    assert_eq!(reset.removed, 2);
    assert!(reset.dir_sync.is_none());
    let actual: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        actual,
        serde_json::json!({
            "app.exit": "ctrl+d", "unknown": {"kept": true}, "multiplexer": 42
        })
    );
    let manager = KeybindingsManager::create(scratch.path().to_str()).unwrap();
    assert!(manager.matches("\u{4}", "app.exit"));
}
