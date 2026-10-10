//! Recorded assembly and metadata behavior.
use crate::Model;
use indexmap::IndexMap;
use serde::Deserialize;
/// A unique controlled query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Failure label.
    id: String,
    /// Owning test.
    test: String,
    /// Owning operation.
    operation: String,
    /// Operation operands.
    input: serde_json::Value,
    /// Complete result.
    expected: serde_json::Value,
    /// Ordered statistics.
    #[serde(default)]
    counts: Option<serde_json::Value>,
}
/// Compare complete metadata for all selected inputs.
fn thinking_cases(name: &str) {
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        assert_eq!(case.operation, "thinking");
        assert!(case.counts.is_none());
        let mut model: Model = serde_json::from_value(case.input).unwrap();
        super::thinking::apply_thinking_level_metadata(&mut model);
        let expected: Model = serde_json::from_value(case.expected).unwrap();
        assert_eq!(model, expected, "{}", case.id);
        count += 1;
    }
    assert!(count > 0);
}
#[test]
fn thinking_predicates_keep_source_order() {
    thinking_cases("thinking_predicates_keep_source_order");
}

#[test]
fn thinking_merges_retain_unmentioned_levels() {
    thinking_cases("thinking_merges_retain_unmentioned_levels");
}

#[test]
fn thinking_metadata_does_not_require_reasoning() {
    thinking_cases("thinking_metadata_does_not_require_reasoning");
}

/// Compare assembled descriptors and ordered provider counts.
fn assembly_cases(name: &str) {
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        assert_eq!(case.operation, "assemble");
        let feeds: [Vec<Model>; 3] = serde_json::from_value(case.input).unwrap();
        let catalog = super::assemble(feeds);

        let expected = expected_models(case.expected);
        let actual = sorted_models(&catalog);
        assert_eq!(actual, expected.iter().collect::<Vec<_>>(), "{}", case.id);
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(
                actual
                    .headers
                    .as_ref()
                    .map(|headers| headers.keys().collect::<Vec<_>>()),
                expected
                    .headers
                    .as_ref()
                    .map(|headers| headers.keys().collect::<Vec<_>>()),
                "{} header order",
                case.id
            );
        }
        let counts: Vec<(String, usize)> = serde_json::from_value(case.counts.unwrap()).unwrap();
        assert_eq!(
            super::provider_counts(&catalog),
            counts,
            "{} counts",
            case.id
        );
        if name == "statistics_use_unique_models_and_encounter_provider_order" {
            assert_statistics(&catalog, &expected, &counts);
        }
        count += 1;
    }
    assert!(count > 0);
}
/// Compare rendered totals and provider order against the independent output records.
fn assert_statistics(catalog: &super::Catalog, expected: &[Model], counts: &[(String, usize)]) {
    use std::fmt::Write as _;
    let mut output = Vec::new();
    super::print_statistics(catalog, &mut output).unwrap();
    let mut report = format!(
        "\nModel Statistics:\n  Total tool-capable models: {}\n  Reasoning-capable models: {}\n",
        expected.len(),
        expected.iter().filter(|model| model.reasoning).count()
    );
    for (provider, count) in counts {
        writeln!(report, "  {provider}: {count} models").unwrap();
    }
    assert_eq!(
        String::from_utf8(output).unwrap(),
        report,
        "rendered unique statistics"
    );
}
#[test]
fn missing_models_receive_every_authored_descriptor() {
    assembly_cases("missing_models_receive_every_authored_descriptor");
}

#[test]
fn conditional_additions_keep_existing_descriptors() {
    assembly_cases("conditional_additions_keep_existing_descriptors");
}

#[test]
fn spark_filter_is_provider_and_id_specific() {
    assembly_cases("spark_filter_is_provider_and_id_specific");
}

#[test]
fn copilot_derivation_requires_base_and_keeps_first() {
    assembly_cases("copilot_derivation_requires_base_and_keeps_first");
}

#[test]
fn deepseek_compat_merge_preserves_unrelated_fields() {
    assembly_cases("deepseek_compat_merge_preserves_unrelated_fields");
}

#[test]
fn minimax_allowlist_prunes_only_direct_providers() {
    assembly_cases("minimax_allowlist_prunes_only_direct_providers");
}

#[test]
fn conditional_additions_match_both_identity_parts() {
    assembly_cases("conditional_additions_match_both_identity_parts");
}

#[test]
fn conditional_additions_and_dedup_keep_first() {
    assembly_cases("conditional_additions_and_dedup_keep_first");
}

#[test]
fn opus_cache_prices_change_only_named_fields() {
    assembly_cases("opus_cache_prices_change_only_named_fields");
}

#[test]
fn opus_cache_first_match_precedes_dedup() {
    assembly_cases("opus_cache_first_match_precedes_dedup");
}

#[test]
fn bedrock_cache_override_uses_substring_and_provider() {
    assembly_cases("bedrock_cache_override_uses_substring_and_provider");
}

#[test]
fn claude_context_overrides_cover_every_provider_spelling() {
    assembly_cases("claude_context_overrides_cover_every_provider_spelling");
}

#[test]
fn older_sonnet_context_override_is_exact() {
    assembly_cases("older_sonnet_context_override_is_exact");
}

#[test]
fn gpt_context_overrides_keep_provider_specific_ids() {
    assembly_cases("gpt_context_overrides_keep_provider_specific_ids");
}

#[test]
fn router_corrections_preserve_unmentioned_prices() {
    assembly_cases("router_corrections_preserve_unmentioned_prices");
}

#[test]
fn feed_combination_preserves_first_wins_identity() {
    assembly_cases("feed_combination_preserves_first_wins_identity");
}

#[test]
fn catalog_identity_is_a_pair_not_a_joined_string() {
    assembly_cases("catalog_identity_is_a_pair_not_a_joined_string");
}

#[test]
fn statistics_use_unique_models_and_encounter_provider_order() {
    assembly_cases("statistics_use_unique_models_and_encounter_provider_order");
}

#[test]
fn azure_derivation_clones_selected_response_models() {
    assembly_cases("azure_derivation_clones_selected_response_models");
}

#[test]
fn prototype_names_are_literal_catalog_keys() {
    assembly_cases("prototype_names_are_literal_catalog_keys");
}

#[test]
fn unconditional_additions_are_first_wins_not_replacements() {
    assembly_cases("unconditional_additions_are_first_wins_not_replacements");
}

/// Own a native temporary generated directory.
struct Scratch(std::path::PathBuf);
impl Scratch {
    /// Allocate an isolated directory with a process-local unique identity.
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn source_emission_sorts_utf16_keys() {
    assembly_cases("source_emission_sorts_utf16_keys");
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus.json")).unwrap();
    let case = cases
        .into_iter()
        .find(|case| case.test == "source_emission_sorts_utf16_keys")
        .unwrap();
    let catalog = super::assemble(serde_json::from_value(case.input).unwrap());
    let scratch = Scratch::new();
    super::emit::write_catalog(&catalog, &scratch.0.join("generated")).unwrap();
    let registry = std::fs::read_to_string(scratch.0.join("generated/mod.rs")).unwrap();
    assert!(registry.find("😀").unwrap() < registry.find("\\u{e000}").unwrap());
    let provider = std::fs::read_to_string(scratch.0.join("generated/provider_a.rs")).unwrap();
    for (left, right) in [("01", "10"), ("10", "2"), ("2", "😀"), ("😀", "\u{e000}")] {
        assert!(
            provider.find(&format!("({left:?},")).unwrap()
                < provider.find(&format!("({right:?},")).unwrap()
        );
    }
}

/// Reuse the existing complete catalog snapshot instead of duplicating it.
fn expected_models(value: serde_json::Value) -> Vec<Model> {
    if value == "offline_catalog" {
        let providers: indexmap::IndexMap<String, indexmap::IndexMap<String, Model>> =
            serde_json::from_str(include_str!(
                "../../../../tests/fixtures/offline_model_catalog.json"
            ))
            .unwrap();
        providers
            .into_values()
            .flat_map(indexmap::IndexMap::into_values)
            .collect()
    } else {
        serde_json::from_value(value).unwrap()
    }
}
mod emission;

/// Borrow models in deterministic emitted order.
fn sorted_models(catalog: &super::Catalog) -> Vec<&Model> {
    let mut models: Vec<_> = catalog.values().flat_map(IndexMap::values).collect();
    models.sort_by(|left, right| {
        super::utf16_order(&left.provider, &right.provider)
            .then_with(|| super::utf16_order(&left.id, &right.id))
    });
    models
}
