//! Compiled author observations through the production loader and native helper owners.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, create_extension_runtime, load_extension_from_factory,
};
use serde_json::Value;
use std::{rc::Rc, sync::RwLock};

#[test]
fn helper_width_keeps_unicode_and_ansi() {
    check("helper_width_keeps_unicode_and_ansi");
}

#[test]
fn helper_terminal_normalization_uses_native_owner() {
    check("helper_terminal_normalization_uses_native_owner");
}

#[test]
fn helper_ansi_offsets_follow_utf8_boundaries() {
    check("helper_ansi_offsets_follow_utf8_boundaries");
}

#[test]
fn helper_character_classes_keep_boundaries() {
    check("helper_character_classes_keep_boundaries");
}

#[test]
fn helper_compiled_guest_calls_production_owners() {
    check("helper_compiled_guest_calls_production_owners");
}

#[test]
fn helper_wrap_preserves_styled_spaces() {
    check("helper_wrap_preserves_styled_spaces");
}

#[test]
fn helper_background_callback_receives_padded_line_once() {
    check("helper_background_callback_receives_padded_line_once");
}

#[test]
fn helper_width_truncation_keeps_options() {
    check("helper_width_truncation_keeps_options");
}

#[test]
fn helper_column_slice_keeps_strict_and_width() {
    check("helper_column_slice_keeps_strict_and_width");
}

#[test]
fn helper_segments_keep_both_widths_and_style() {
    check("helper_segments_keep_both_widths_and_style");
}

#[test]
fn helper_key_vocabulary_and_modifiers_match_host() {
    check("helper_key_vocabulary_and_modifiers_match_host");
}

#[test]
fn helper_keyboard_state_is_shared_with_host() {
    check("helper_keyboard_state_is_shared_with_host");
}

#[test]
fn helper_key_decoding_forwards_all_result_branches() {
    check("helper_key_decoding_forwards_all_result_branches");
}

#[test]
fn helper_key_matching_keeps_modifier_negatives() {
    check("helper_key_matching_keeps_modifier_negatives");
}

#[test]
fn helper_fuzzy_match_keeps_scores_and_swap() {
    check("helper_fuzzy_match_keeps_scores_and_swap");
}

#[test]
fn helper_fuzzy_filter_keeps_identity_and_getter_order() {
    check("helper_fuzzy_filter_keeps_identity_and_getter_order");
}

#[test]
fn helper_fuzzy_callbacks_keep_live_edits() {
    check("helper_fuzzy_callbacks_keep_live_edits");
}

#[test]
fn helper_frontmatter_preserves_document_kinds() {
    check("helper_frontmatter_preserves_document_kinds");
}

#[test]
fn helper_raw_yaml_preserves_document_values() {
    check("helper_raw_yaml_preserves_document_values");
}

#[test]
fn helper_strip_frontmatter_keeps_parse_failures() {
    check("helper_strip_frontmatter_keeps_parse_failures");
}

#[test]
fn helper_adapter_swaps_without_author_changes() {
    check("helper_adapter_swaps_without_author_changes");
}

#[test]
fn helper_tool_truncation_keeps_all_accounting() {
    check("helper_tool_truncation_keeps_all_accounting");
}

#[test]
fn helper_line_truncation_keeps_default_and_flag() {
    check("helper_line_truncation_keeps_default_and_flag");
}

#[test]
fn helper_size_display_keeps_unit_boundaries() {
    check("helper_size_display_keeps_unit_boundaries");
}

#[test]
fn helper_truncation_defaults_come_from_host() {
    check("helper_truncation_defaults_come_from_host");
}

#[test]
fn helper_paths_use_native_expansion_and_resolution() {
    check("helper_paths_use_native_expansion_and_resolution");
}

#[test]
fn helper_read_path_uses_existing_spelling() {
    check("helper_read_path_uses_existing_spelling");
}

#[test]
fn helper_conversation_conversion_keeps_each_role() {
    check("helper_conversation_conversion_keeps_each_role");
}

#[test]
fn helper_conversation_serialization_keeps_text_and_tools() {
    check("helper_conversation_serialization_keeps_text_and_tools");
}

#[test]
fn helper_agent_directory_reads_installation_identity() {
    check("helper_agent_directory_reads_installation_identity");
}

#[test]
fn helper_version_reads_installation_identity() {
    check("helper_version_reads_installation_identity");
}

/// Executes every recorded row of one behavior through the unchanged author fixture.
fn check(name: &str) {
    if (name.starts_with("helper_paths_") || name == "helper_read_path_uses_existing_spelling")
        && isolate_paths(name)
    {
        return;
    }
    let scratch = path_fixture();
    if name.starts_with("helper_key") && maestro_test_process::run_alone(name).unwrap() {
        return;
    }
    let original = maestro_tui::is_kitty_protocol_active();
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let runtime = create_extension_runtime::<Local>();
        let path = std::env::var("MAESTRO_HELPER_COMPONENT").unwrap();
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            "/helpers".into(),
            bus.controller.for_extension(),
            runtime.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner.clone()])),
            runtime,
            super::session::runner_session("/helpers").0,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        let handler = runner.get_command("helpers").unwrap().command.handler;
        let mut count = 0;
        for mut row in runnable_rows(name, &scratch) {
            expand(
                &mut row,
                scratch.path().to_str().unwrap(),
                &std::env::var("HOME").unwrap_or_default(),
            );
            let result = handler(row.to_string(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string();
            let observed: Value =
                serde_json::from_str(&result).unwrap_or_else(|e| panic!("{e}: {result}"));
            assert_eq!(observed, row["ok"], "{row}");
            if let Some(active) = row["context"]["kittyProtocolActive"].as_bool() {
                assert_eq!(maestro_tui::is_kitty_protocol_active(), active);
            }
            count += 1;
        }
        assert_ne!(count, 0);
        drop(owner);
        drop(runner);
        bus.controller.clear();
        maestro_tui::set_kitty_protocol_active(original);
    });
}

/// The recorded rows of one behavior that this host can exercise.
///
/// The fixture stores some names decomposed. A host that folds canonically equivalent names
/// finds them under the precomposed spelling a row authors, so a row that reaches a decomposed
/// name from a spelling without the combining mark states that premise instead.
fn runnable_rows(name: &str, scratch: &maestro_test_directory::Scratch) -> Vec<Value> {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/helper_corpus.json")).unwrap();
    let pair = ["\u{e9}", "e\u{301}"].map(std::ffi::OsStr::new);
    let folds = !scratch.keeps_distinct(&pair).unwrap();
    let decomposed = |value: &Value| value.as_str().is_some_and(|text| text.contains('\u{301}'));
    (rows.into_iter())
        .filter(|row| row["test"] == name)
        .filter(|row| {
            let refused = folds && decomposed(&row["ok"]) && !decomposed(&row["args"][0]);
            if refused {
                eprintln!("{name} row needs a host that keeps {pair:?} apart: {row}");
            }
            !refused
        })
        .collect()
}

/// Runs directory observations under a supplied host home, absent from the guest environment.
fn isolate_paths(name: &str) -> bool {
    let home = maestro_test_directory::Scratch::new().unwrap();
    let component = std::env::var("MAESTRO_HELPER_COMPONENT").unwrap();
    let home_text = home.path().to_str().unwrap().to_owned();
    maestro_test_process::run_per_environment(name, [home_text], |home| {
        let mut variables = std::collections::BTreeMap::from([
            ("HOME", home.clone()),
            ("USERPROFILE", home.clone()),
            ("MAESTRO_HELPER_COMPONENT", component.clone()),
        ]);
        variables.extend(
            maestro_test_process::user_cache_home()
                .map(|path| ("XDG_CACHE_HOME", path.to_string_lossy().into_owned())),
        );
        variables
    })
    .unwrap()
    .is_none()
}
/// Expands only named fixture placeholders, leaving authored path spellings intact.
///
/// `${SEP}` marks a separator that resolution writes, so it takes the host's spelling.
fn expand(value: &mut Value, root: &str, home: &str) {
    match value {
        Value::String(text) => {
            *text = text
                .replace("${ROOT}", root)
                .replace("${HOME}", home)
                .replace("${SEP}", std::path::MAIN_SEPARATOR_STR);
        }
        Value::Array(items) => {
            for item in items {
                expand(item, root, home);
            }
        }
        Value::Object(entries) => {
            for item in entries.values_mut() {
                expand(item, root, home);
            }
        }
        _ => {}
    }
}

/// Existing spelling candidates live only in this exclusive disposable root.
fn path_fixture() -> maestro_test_directory::Scratch {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    for name in [
        "g’e\u{301}",
        "café.txt",
        "space name",
        "shot 10\u{202f}AM.png",
        "shot 10\u{202f}pm.png",
        "decompose\u{301}.png",
        "capture d’écran.png",
        "de\u{301}cran d’e\u{301}cran.png",
    ] {
        std::fs::write(scratch.path().join(name), b"fixture").unwrap();
    }
    scratch
}
