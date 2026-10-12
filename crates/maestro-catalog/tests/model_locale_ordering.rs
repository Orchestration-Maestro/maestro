#![cfg(test)]
//! Complete native model ordering with explicit and process locale operands.
#[path = "support/descriptors.rs"]
mod descriptors;
mod selection;
#[path = "support/subprocess.rs"]
mod subprocess;
use maestro_catalog::{model_id_comparator, parse_model_pattern};
use serde::Deserialize;
use std::collections::HashMap;
/// A complete recorded native ordering.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocaleCase {
    /// Present locale operands.
    environment: HashMap<String, String>,
    /// Supplied catalog identity order.
    input: Vec<Identity>,
    /// Expected complete descending identity order.
    order: Vec<Identity>,
}
/// Identity independent of fixed descriptor metadata.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Identity {
    /// Model ID compared by the native collator.
    id: String,
    /// Distinct provider retaining equal-ID/collation order.
    provider: String,
}
/// Read the strict, complete native corpus.
fn cases() -> Vec<LocaleCase> {
    serde_json::from_str(include_str!("fixtures/model_locale_ordering.json")).unwrap()
}
/// Observe descending public selection until the catalog is exhausted.
fn check_order(case: &LocaleCase, reader: &dyn Fn(&str) -> Option<String>) {
    let compare = model_id_comparator(reader).unwrap();
    let mut handles: Vec<_> = case
        .input
        .iter()
        .map(|m| selection::descriptor(&m.id, &m.provider))
        .collect();
    let mut order = Vec::new();
    while !handles.is_empty() {
        let result = parse_model_pattern(
            "",
            &handles,
            maestro_catalog::ParseModelPatternOptions::default(),
            &compare,
        )
        .model
        .unwrap();
        let model = result.read().unwrap();
        order.push(Identity {
            id: model.id.clone(),
            provider: model.provider.clone(),
        });
        let index = handles
            .iter()
            .position(|h| std::sync::Arc::ptr_eq(h, &result))
            .unwrap();
        handles.remove(index);
    }
    assert_eq!(order, case.order);
}
#[test]
fn controlled_collation_drives_public_selection() {
    selection::replay(&["case_169"]);
}
/// Child fixture index, absent in the parent runner.
fn child_index() -> Option<usize> {
    std::env::var("MAESTRO_LOCALE_CASE")
        .ok()
        .map(|s| s.parse().unwrap())
}
/// Run one recorded environment, removing absent sibling operands.
fn run_locale_child(name: &str, index: usize, environment: &HashMap<String, String>) {
    let index = index.to_string();
    let mut operands: Vec<_> = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .map(|key| (key, environment.get(key).map(String::as_str)))
        .collect();
    operands.push(("MAESTRO_LOCALE_CASE", Some(&index)));
    subprocess::run_child(name, &operands);
}
#[test]
fn native_model_ordering_tracks_process_locale() {
    let cases = cases();
    if let Some(index) = child_index() {
        check_order(&cases[index], &|key| std::env::var(key).ok());
        return;
    }
    for (index, case) in cases.iter().enumerate() {
        run_locale_child(
            "native_model_ordering_tracks_process_locale",
            index,
            &case.environment,
        );
    }
}
#[test]
fn supplied_locale_environment_overrides_process_environment() {
    let cases = cases();
    if let Some(index) = child_index() {
        check_order(&cases[index], &|key| {
            cases[index].environment.get(key).cloned()
        });
        return;
    }
    for (index, case) in cases.iter().enumerate() {
        let decoy = if case.environment.values().any(|v| v.starts_with("sv")) {
            "en_US"
        } else {
            "sv_SE"
        };
        run_locale_child(
            "supplied_locale_environment_overrides_process_environment",
            index,
            &HashMap::from([("LC_ALL".into(), decoy.into())]),
        );
    }
}
#[test]
fn native_model_ordering_recovers_from_invalid_locale() {
    if child_index().is_some() {
        check_order(&cases()[0], &|key| std::env::var(key).ok());
        return;
    }
    run_locale_child(
        "native_model_ordering_recovers_from_invalid_locale",
        0,
        &HashMap::from([("LC_ALL".into(), "not a locale!".into())]),
    );
}
