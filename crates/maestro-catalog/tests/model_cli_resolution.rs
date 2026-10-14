#![cfg(test)]
//! Command-line model selection and mutable provider defaults.
#[path = "support/descriptors.rs"]
mod descriptors;
#[path = "support/isolation.rs"]
mod isolation;
mod selection;
use selection::replay;
#[test]
fn cli_absent_model_bypasses_catalog_validation() {
    replay(&["case_142", "case_143"]);
}

#[test]
fn cli_catalog_and_provider_errors_keep_precedence() {
    replay(&["case_028", "case_144", "case_145", "case_146", "case_175"]);
}

#[test]
fn cli_prefers_known_provider_before_raw_gateway_id() {
    replay(&["case_022", "case_029", "case_148", "case_149", "case_157"]);
}

#[test]
fn cli_exact_raw_matches_preserve_catalog_order() {
    replay(&["case_150", "case_151"]);
}

#[test]
fn cli_explicit_provider_constrains_partial_matches() {
    replay(&["case_023", "case_030", "case_152"]);
}

#[test]
fn cli_returns_thinking_without_applying_it() {
    replay(&["case_024", "case_168"]);
}

#[test]
fn cli_provider_lookup_uses_last_case_spelling() {
    replay(&["case_147"]);
}

#[test]
fn cli_inferred_failure_retries_full_raw_input() {
    replay(&["case_025", "case_155", "case_173"]);
}

#[test]
fn cli_custom_ids_retain_default_or_first_metadata() {
    replay(&[
        "case_026", "case_156", "case_158", "case_162", "case_163", "case_164", "case_176",
    ]);
}

#[test]
fn cli_missing_models_report_exact_text() {
    replay(&["case_159"]);
}

#[test]
fn cli_partial_matching_uses_untrimmed_model_input() {
    replay(&["case_160", "case_161", "case_172"]);
}

#[test]
fn cli_strips_only_one_explicit_provider_prefix() {
    replay(&["case_027", "case_153", "case_154", "case_177"]);
}

#[test]
fn provider_defaults_preserve_order_values_and_live_edits() {
    if isolation::isolated("provider_defaults_preserve_order_values_and_live_edits") {
        return;
    }
    replay(&["case_031", "case_165", "case_166", "case_167"]);
}
#[test]
fn custom_models_are_detached_without_registry_mutation() {
    use maestro_catalog::{ResolveCliModelOptions, resolve_cli_model};
    use std::sync::Arc;
    let source = selection::descriptor("source", "unconfigured-provider");
    let registry = selection::registry(&[Arc::clone(&source)]);
    let result = resolve_cli_model(
        ResolveCliModelOptions {
            cli_provider: Some("unconfigured-provider"),
            cli_model: Some("detached"),
            model_registry: &registry,
        },
        &str::cmp,
    )
    .model
    .unwrap();
    assert!(!Arc::ptr_eq(&source, &result));
    let mut expected = source.read().unwrap().clone();
    expected.id = "detached".into();
    expected.name = "detached".into();
    assert!(result.read().unwrap().eq(&expected));
    result.write().unwrap().name = "custom edit".into();
    assert_eq!(source.read().unwrap().name, "source");
    let all = registry.get_all();
    let all = all.read().unwrap();
    assert_eq!(all.len(), 1);
    assert!(Arc::ptr_eq(&all[0], &source));
}
#[test]
fn cli_selection_includes_unconfigured_models() {
    use maestro_catalog::{ResolveCliModelOptions, resolve_cli_model};
    use std::sync::Arc;
    let handle = selection::descriptor("unconfigured", "unconfigured-provider");
    let registry = selection::registry(&[Arc::clone(&handle)]);
    assert!(registry.get_available().unwrap().is_empty());
    let result = resolve_cli_model(
        ResolveCliModelOptions {
            cli_provider: None,
            cli_model: Some("unconfigured"),
            model_registry: &registry,
        },
        &str::cmp,
    );
    assert!(
        result.model.is_some(),
        "all-descriptor selection must include the unconfigured model"
    );
    let selected = result.model.unwrap();
    assert!(Arc::ptr_eq(&selected, &handle));
}
#[test]
fn cli_prefix_stripping_uses_authored_unicode_boundary() {
    use maestro_catalog::{ResolveCliModelOptions, resolve_cli_model};
    use std::sync::Arc;
    let model = selection::descriptor("model", "i\u{307}");
    let decoy = selection::descriptor("z-odel", "i\u{307}");
    let registry = selection::registry(&[Arc::clone(&model), decoy]);
    let result = resolve_cli_model(
        ResolveCliModelOptions {
            cli_provider: Some("i\u{307}"),
            cli_model: Some("İ/model"),
            model_registry: &registry,
        },
        &str::cmp,
    );
    assert!(
        Arc::ptr_eq(&result.model.unwrap(), &model),
        "strip the authored prefix, not the canonical spelling's length"
    );
}
#[test]
fn cli_explicit_slash_provider_strips_prefix_before_partial_selection() {
    use maestro_catalog::{ResolveCliModelOptions, resolve_cli_model};
    use std::sync::Arc;
    let model = selection::descriptor("friendly-model", "team/proxy");
    let registry = selection::registry(&[Arc::clone(&model)]);
    let result = resolve_cli_model(
        ResolveCliModelOptions {
            cli_provider: Some("TEAM/PROXY"),
            cli_model: Some("team/proxy/friend"),
            model_registry: &registry,
        },
        &str::cmp,
    );
    assert!(Arc::ptr_eq(&result.model.unwrap(), &model));
    assert_eq!(result.warning, None);
    assert!(result.error.is_none());
}
#[test]
fn cli_explicit_slash_provider_strips_once_before_custom_fallback() {
    use maestro_catalog::{ResolveCliModelOptions, resolve_cli_model};
    use std::sync::Arc;
    let model = selection::descriptor("friendly-model", "team/proxy");
    let registry = selection::registry(&[Arc::clone(&model)]);
    for (input, id) in [
        ("team/proxy/unknown", "unknown"),
        ("team/proxy/team/proxy/unknown", "team/proxy/unknown"),
    ] {
        let result = resolve_cli_model(
            ResolveCliModelOptions {
                cli_provider: Some("team/proxy"),
                cli_model: Some(input),
                model_registry: &registry,
            },
            &str::cmp,
        );
        let custom = result.model.unwrap();
        assert!(!Arc::ptr_eq(&custom, &model));
        let mut expected = model.read().unwrap().clone();
        expected.id = id.into();
        expected.name = id.into();
        assert_eq!(*custom.read().unwrap(), expected);
        assert_eq!(
            result.warning,
            Some(format!(
                "Model \"{id}\" not found for provider \"team/proxy\". Using custom model id."
            ))
        );
        assert!(result.error.is_none());
    }
}
