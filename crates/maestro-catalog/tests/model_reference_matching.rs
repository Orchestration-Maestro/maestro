#![cfg(test)]
//! Model reference and thinking pattern observations.
use crate::selection;
use selection::replay;
#[test]
fn exact_reference_requires_unique_stage_match() {
    replay(&[
        "case_071", "case_072", "case_073", "case_074", "case_075", "case_076", "case_077",
        "case_078", "case_079", "case_080",
    ]);
}

#[test]
fn exact_reference_trims_authored_whitespace() {
    replay(&[
        "case_032", "case_033", "case_034", "case_035", "case_036", "case_037", "case_038",
        "case_039", "case_040", "case_041", "case_042", "case_043", "case_044", "case_045",
        "case_046", "case_047", "case_048", "case_049", "case_050", "case_051", "case_052",
        "case_053", "case_054", "case_055", "case_056", "case_057", "case_058", "case_059",
        "case_060", "case_061", "case_062", "case_063", "case_064", "case_065", "case_066",
        "case_067", "case_068", "case_069", "case_070",
    ]);
}

#[test]
fn exact_reference_folds_unicode_case() {
    replay(&["case_081", "case_082", "case_083", "case_084"]);
}

#[test]
fn pattern_whole_input_precedes_suffix_parsing() {
    replay(&[
        "case_001", "case_013", "case_014", "case_017", "case_085", "case_086", "case_087",
        "case_171",
    ]);
}

#[test]
fn pattern_nested_suffixes_preserve_warning_precedence() {
    replay(&[
        "case_011", "case_012", "case_018", "case_019", "case_021", "case_104", "case_105",
        "case_106", "case_107", "case_108", "case_109", "case_110", "case_111", "case_112",
        "case_113", "case_114", "case_115", "case_116", "case_117", "case_118", "case_126",
        "case_174", "case_178", "case_179",
    ]);
}

#[test]
fn pattern_accepts_each_thinking_suffix() {
    replay(&[
        "case_004", "case_005", "case_006", "case_007", "case_008", "case_009", "case_010",
        "case_015", "case_016", "case_170",
    ]);
}

#[test]
fn pattern_partial_matching_keeps_authored_spacing() {
    replay(&[
        "case_002", "case_088", "case_089", "case_090", "case_091", "case_092", "case_093",
    ]);
}

#[test]
fn pattern_prefers_aliases_over_eight_digit_versions() {
    replay(&[
        "case_094", "case_095", "case_096", "case_097", "case_098", "case_099",
    ]);
}

#[test]
fn pattern_preserves_absence_and_empty_input_selection() {
    replay(&["case_003", "case_020", "case_103"]);
}

#[test]
fn pattern_selects_descending_locale_id_and_first_tie() {
    replay(&["case_100", "case_101", "case_102"]);
}

#[test]
fn pattern_strict_mode_keeps_invalid_suffixes_unmatched() {
    replay(&[
        "case_119", "case_120", "case_121", "case_122", "case_123", "case_124", "case_125",
        "case_180",
    ]);
}
#[test]
fn selected_models_keep_shared_identity() {
    use maestro_catalog::{
        ResolveCliModelOptions, find_exact_model_reference_match, parse_model_pattern,
        resolve_cli_model,
    };
    use std::sync::Arc;
    let handle = selection::descriptor("shared", "unconfigured-provider");
    let weak = Arc::downgrade(&handle);
    let registry = selection::registry(&[Arc::clone(&handle)]);
    let all = registry.get_all();
    let handles = all.read().unwrap().clone();
    let exact = find_exact_model_reference_match("shared", &handles).unwrap();
    let pattern = parse_model_pattern(
        "share",
        &handles,
        maestro_catalog::ParseModelPatternOptions::default(),
        &str::cmp,
    )
    .model
    .unwrap();
    let cli = resolve_cli_model(
        ResolveCliModelOptions {
            cli_provider: None,
            cli_model: Some("shared"),
            model_registry: &registry,
        },
        &str::cmp,
    )
    .model
    .unwrap();
    handle.write().unwrap().name = "edited through surviving alias".into();
    all.write().unwrap().clear();
    for result in [&exact, &pattern, &cli] {
        assert!(Arc::ptr_eq(result, &handle));
        assert_eq!(
            result.read().unwrap().name,
            "edited through surviving alias"
        );
    }
    drop((exact, pattern, cli, handle, handles, registry, all));
    assert!(weak.upgrade().is_none());
}
