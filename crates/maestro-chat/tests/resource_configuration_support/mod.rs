//! Strict recorded configuration scenarios through the exported component.
mod actions;
mod harness;
#[cfg(unix)]
pub mod native;
pub mod protocol;
mod records;
use harness::Harness;
use records::{Expected, Input};

/// Runs all and only scenarios owned by the requested public witness.
pub fn group(name: &str) {
    // The recorded relative patterns were observed from the working directory `/tmp`, which
    // belongs to the whole process.
    #[cfg(unix)]
    {
        if maestro_test_process::run_alone(name).unwrap() {
            return;
        }
        std::env::set_current_dir("/tmp").unwrap();
    }
    let _guard = super::keybindings::bindings_guard();
    let windows = name == "resource_windows_paths_keep_authored_relative_patterns";
    let (source, owners): (&str, &[&str]) = if windows {
        (
            include_str!("../fixtures/resource_configuration_windows.json"),
            &["resource_windows_paths_keep_authored_relative_patterns"],
        )
    } else {
        (
            include_str!("../fixtures/resource_configuration.json"),
            &OWNERS,
        )
    };
    let rows = super::selector::load(source);
    assert_eq!(rows.len(), if windows { 4 } else { 152 });
    for (input, expected) in super::selector::cases::<Input, Expected>(rows, owners, name) {
        #[cfg(unix)]
        if CWD_SPELLED.contains(&name)
            && needs_recorded_cwd(&input)
            && std::env::current_dir().unwrap() != std::path::Path::new("/tmp")
        {
            eprintln!("{name}: only this relative-operand row needs `/tmp` to read back as itself");
            continue;
        }
        if name.starts_with("malformed_") {
            qualify_adapters(&input.settings);
        }
        replay(input, &expected);
    }
}
#[cfg(unix)]
#[test]
fn absolute_resource_patterns_do_not_depend_on_temporary_directory_spelling() {
    let _guard = super::keybindings::bindings_guard();
    let rows = super::selector::load(include_str!("../fixtures/resource_configuration.json"));
    let (input, expected) = super::selector::cases::<Input, Expected>(
        rows,
        &OWNERS,
        "resource_patterns_replace_exact_signed_entries_in_order",
    )
    .into_iter()
    .next()
    .unwrap();
    replay(input, &expected);
}

/// Identify relative path operands in the groups recorded from `/tmp`.
#[cfg(unix)]
fn needs_recorded_cwd(input: &Input) -> bool {
    input
        .resolved
        .extensions
        .iter()
        .chain(&input.resolved.skills)
        .chain(&input.resolved.prompts)
        .chain(&input.resolved.themes)
        .any(|resource| {
            !std::path::Path::new(&resource.path).is_absolute()
                || resource
                    .metadata
                    .base_dir
                    .as_ref()
                    .is_some_and(|base| !std::path::Path::new(base).is_absolute())
        })
}

/// Replays each operation against its own complete independent observation.
fn replay(input: Input, expected: &Expected) {
    let width = input.width;
    let operations = input.operations;
    let harness = Harness::new(
        input.resolved.native(),
        &input.settings,
        (&input.cwd, &input.agent_dir),
        input.styles_enabled.unwrap_or(true),
    );
    let list = harness.component.get_resource_list().clone();
    let weak = std::rc::Rc::downgrade(&list);
    assert!(std::rc::Rc::ptr_eq(
        &list,
        harness.component.get_resource_list()
    ));
    assert_eq!(expected.frames.len(), operations.len() + 1);
    assert_eq!(expected.outcomes.len(), operations.len());
    assert_eq!(harness.frame(width), expected.frames[0]);
    for (index, operation) in operations.into_iter().enumerate() {
        harness.apply(operation, width);
        assert_eq!(expected.outcomes[index], "success");
        assert_eq!(harness.frame(width), expected.frames[index + 1]);
    }
    harness.persisted(&expected.persisted);
    drop(list);
    drop(harness);
    assert!(weak.upgrade().is_none(), "last list alias released");
}
/// The groups whose recorded patterns depend on the working directory's spelling.
#[cfg(unix)]
const CWD_SPELLED: [&str; 2] = [
    "package_patterns_use_supplied_base_or_parent_fallback",
    "resource_patterns_replace_exact_signed_entries_in_order",
];
/// Fixed ownership inventory, independent of fixture labels.
const OWNERS: [&str; 23] = [
    "resource_groups_keep_scope_labels_and_kind_order",
    "mixed_scopes_have_consistent_group_order",
    "resource_names_use_basename_rules_and_stable_locale_order",
    "resource_navigation_skips_headings_and_clamps",
    "resource_page_keys_use_flat_window_offsets",
    "resource_filter_matches_name_kind_and_full_path",
    "resource_search_preserves_query_whitespace_semantics",
    "top_level_toggles_write_only_selected_scope_and_kind",
    "resource_patterns_replace_exact_signed_entries_in_order",
    "package_toggles_match_first_source_and_preserve_members",
    "package_patterns_use_supplied_base_or_parent_fallback",
    "temporary_toggles_use_the_existing_user_scope_branch",
    "toggle_notifications_observe_committed_rows_and_settings",
    "resource_callback_slots_are_replaceable_and_reentrant",
    "item_updates_keep_shared_identity_and_filter_replay",
    "config_selector_retains_its_focus_list_and_child_array",
    "resource_chrome_and_rows_keep_exact_text_and_visible_styles",
    "resource_input_uses_live_bindings_and_literal_space",
    "malformed_resource_lists_keep_unknown_values_while_toggling",
    "malformed_package_sources_keep_unknown_data_while_toggling",
    "resource_scope_writes_survive_flush_and_reload",
    "resource_tabs_expand_without_losing_inverse_spaces",
    "detached_item_updates_keep_first_matching_path_and_kind",
];

/// Qualifies the newly public adapter paths separately from selector persistence.
fn qualify_adapters(scopes: &records::Scopes) {
    use maestro_app::{PackageSource, SettingsListEntry};
    for (kind, value) in [&scopes.global, &scopes.project]
        .into_iter()
        .flat_map(|scope| scope.as_object().unwrap())
    {
        let Some(entries) = value.as_array() else {
            continue;
        };
        for entry in entries {
            let restored = if kind == "packages" {
                let package: PackageSource = PackageSource::from_value(entry);
                package.into_value()
            } else {
                let member: SettingsListEntry = SettingsListEntry::from_value(entry);
                member.into_value()
            };
            assert_eq!(
                restored.to_string(),
                entry.to_string(),
                "public lossless adapter"
            );
        }
    }
}
