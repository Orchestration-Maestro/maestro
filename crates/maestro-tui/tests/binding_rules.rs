use maestro_tui::KeybindingKeys;

#[test]
fn default_definitions_keep_every_key_and_description() {
    binding_support::run("default_definitions_keep_every_key_and_description");
}

mod binding_support;
#[test]
fn maestro_keys_does_not_evict_selector_confirm_when_input_submit_is_rebound() {
    binding_support::run(
        "maestro_keys_does_not_evict_selector_confirm_when_input_submit_is_rebound",
    );
}

#[test]
fn maestro_keys_does_not_evict_cursor_bindings_when_another_action_reuses_the_same_key() {
    binding_support::run(
        "maestro_keys_does_not_evict_cursor_bindings_when_another_action_reuses_the_same_key",
    );
}

#[test]
fn maestro_keys_still_reports_direct_user_binding_conflicts_without_evicting_defaults() {
    binding_support::run(
        "maestro_keys_still_reports_direct_user_binding_conflicts_without_evicting_defaults",
    );
}

#[test]
fn key_lists_deduplicate_without_changing_definition_shapes() {
    binding_support::run("key_lists_deduplicate_without_changing_definition_shapes");
}

#[test]
fn overrides_distinguish_absent_undefined_empty_and_single() {
    binding_support::run("overrides_distinguish_absent_undefined_empty_and_single");
}

#[test]
fn unknown_actions_remain_in_user_data_but_not_resolution() {
    binding_support::run("unknown_actions_remain_in_user_data_but_not_resolution");
}

#[test]
fn unsupported_key_identifiers_are_retained_but_never_match() {
    binding_support::run("unsupported_key_identifiers_are_retained_but_never_match");
}

#[test]
fn conflicts_compare_literal_keys_not_decoder_equivalence() {
    binding_support::run("conflicts_compare_literal_keys_not_decoder_equivalence");
}

#[test]
fn conflicts_keep_first_key_claim_and_claimant_order() {
    binding_support::run("conflicts_keep_first_key_claim_and_claimant_order");
}

#[test]
fn defaults_and_single_user_claims_do_not_report_conflicts() {
    binding_support::run("defaults_and_single_user_claims_do_not_report_conflicts");
}

#[test]
fn empty_and_undefined_overrides_do_not_claim_keys() {
    binding_support::run("empty_and_undefined_overrides_do_not_claim_keys");
}

#[test]
fn empty_definitions_leave_only_the_user_snapshot() {
    binding_support::run("empty_definitions_leave_only_the_user_snapshot");
}

#[test]
fn custom_actions_and_empty_action_ids_are_open_data() {
    binding_support::run("custom_actions_and_empty_action_ids_are_open_data");
}

#[test]
fn repeated_record_entries_replace_value_without_moving_slot() {
    binding_support::run("repeated_record_entries_replace_value_without_moving_slot");
}

#[test]
fn numeric_action_ids_keep_insertion_order() {
    binding_support::run("numeric_action_ids_keep_insertion_order");
}

#[test]
fn prototype_named_actions_are_ordinary_record_entries() {
    binding_support::run("prototype_named_actions_are_ordinary_record_entries");
}

#[test]
fn replacing_user_bindings_rebuilds_keys_and_conflicts() {
    binding_support::run("replacing_user_bindings_rebuilds_keys_and_conflicts");
}

#[test]
fn replacing_with_empty_configuration_restores_defaults() {
    binding_support::run("replacing_with_empty_configuration_restores_defaults");
}

/// Restores the protocol flag even if an assertion unwinds.
struct ProtocolRestore(bool);
impl Drop for ProtocolRestore {
    fn drop(&mut self) {
        maestro_tui::set_kitty_protocol_active(self.0);
    }
}
#[test]
fn matching_reads_current_protocol_and_does_not_filter_releases() {
    let _restore = ProtocolRestore(maestro_tui::is_kitty_protocol_active());
    let corpus = binding_support::corpus();
    let cases = corpus["cases"].as_array().unwrap();
    let first = cases.iter().find(|c| c["id"] == "matching-false").unwrap();
    let manager = binding_support::manager(&first["input"]);
    for id in ["matching-false", "matching-true"] {
        let case = cases.iter().find(|c| c["id"] == id).unwrap();
        maestro_tui::set_kitty_protocol_active(case["input"]["kitty"].as_bool().unwrap());
        assert_eq!(
            binding_support::observe(&manager, &case["expected"]["before"]),
            case["expected"]["before"]
        );
    }
}

#[test]
fn returned_snapshots_cannot_change_manager_state() {
    use serde_json::json;
    for (id, input) in [
        (
            "copies",
            json!({"definitions": [["a", {"defaultKeys": ["left", "right"]}]], "user": [["a", ["enter", "tab"]], ["unknown", null]]}),
        ),
        (
            "conflict-copy",
            json!({"definitions": [["a", {"defaultKeys": "left"}], ["b", {"defaultKeys": "right"}]], "user": [["a", "ctrl+x"], ["b", "ctrl+x"]]}),
        ),
    ] {
        let manager = binding_support::manager(&input);
        let mut keys = manager.get_keys("a");
        keys.clear();
        let mut user = manager.get_user_bindings();
        if let Some(KeybindingKeys::Multiple(keys)) = &mut user[0].1 {
            keys.push("changed".into());
        } else {
            user[0].1 = Some(KeybindingKeys::Single("changed".into()));
        }
        user.push(("extra".into(), Some(KeybindingKeys::Single("tab".into()))));
        let mut resolved = manager.get_resolved_bindings();
        if let Some(KeybindingKeys::Multiple(keys)) = &mut resolved[0].1 {
            keys.clear();
        } else {
            resolved[0].1 = Some(KeybindingKeys::Single("changed".into()));
        }
        let mut definition = manager.get_definition("a").unwrap();
        if let KeybindingKeys::Multiple(keys) = &mut definition.default_keys {
            keys.push("changed".into());
        } else {
            definition.default_keys = KeybindingKeys::Single("changed".into());
        }
        let mut conflicts = manager.get_conflicts();
        if let Some(conflict) = conflicts.first_mut() {
            conflict.key = "changed".into();
            conflict.keybindings.clear();
        }
        conflicts.clear();
        let corpus = binding_support::corpus();
        let expected = &corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap()["expected"];
        assert_eq!(
            binding_support::observe(&manager, expected),
            *expected,
            "{id}"
        );
    }
}
