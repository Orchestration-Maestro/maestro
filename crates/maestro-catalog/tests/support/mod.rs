#![cfg(test)]
//! Controlled files and source-derived expected catalog changes.
use maestro_catalog::ModelRegistry;
use maestro_models::{Model, get_models, get_providers};
mod common;
pub use common::*;
use serde::Deserialize;
use std::collections::HashMap;

/// One independently recorded file input and its public observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: String,
    expected: Expected,
}
/// Expected differences from the offline inventory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(deserialize_with = "descriptors")]
    changes: Vec<Model>,
    error: Option<Error>,
    keys: Option<Vec<(String, String)>>,
    /// Keys following the unchanged offline inventory, in order.
    appended: Option<Vec<(String, String)>>,
}
/// Source diagnostics omit platform-dependent parser details.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Error {
    full: Option<String>,
    kind: Option<String>,
}
/// Replay one fixture group through both real and controlled file operations.
pub fn corpus(group: &str) {
    let cases: HashMap<String, Vec<Case>> =
        serde_json::from_str(include_str!("catalog-fixtures.json")).unwrap();
    let baseline: HashMap<_, _> = get_providers()
        .iter()
        .flat_map(|p| get_models(p))
        .map(|m| ((m.provider.clone(), m.id.clone()), m))
        .collect();
    for (index, case) in cases.get(group).expect("fixture group").iter().enumerate() {
        #[cfg(not(target_arch = "wasm32"))]
        let file = File::new(case.input.as_bytes());
        #[cfg(not(target_arch = "wasm32"))]
        let registries = vec![
            (registry(&case.input), "<models-file>"),
            (
                ModelRegistry::create(auth(), file.path()).unwrap(),
                file.path(),
            ),
        ];
        #[cfg(target_arch = "wasm32")]
        let registries = vec![(registry(&case.input), "<models-file>")];
        for (registry, path) in registries {
            check_case(&registry, &baseline, &case.expected, path, (group, index));
        }
    }
}
/// Compare the complete key sequence, whole or after the offline inventory.
fn check_keys(actual: &[Model], expected: &Expected, index: usize) {
    let actual: Vec<_> = actual
        .iter()
        .map(|m| (m.provider.clone(), m.id.clone()))
        .collect();
    if let Some(keys) = &expected.keys {
        assert_eq!(actual, *keys, "key order case {index}");
    }
    if let Some(appended) = &expected.appended {
        let mut keys: Vec<_> = get_providers()
            .iter()
            .flat_map(|p| get_models(p))
            .map(|m| (m.provider, m.id))
            .collect();
        keys.extend(appended.iter().cloned());
        assert_eq!(actual, keys, "inventory order case {index}");
    }
}
/// Compare the complete output's changed descriptors, optional key sequence and error.
fn check_case(
    registry: &ModelRegistry,
    baseline: &HashMap<(String, String), Model>,
    expected: &Expected,
    path: &str,
    label: (&str, usize),
) {
    let (group, index) = label;
    let all = registry.get_all();
    let all = all.read().unwrap();
    let actual: Vec<_> = all.iter().map(|m| m.read().unwrap().clone()).collect();
    let changes: Vec<_> = actual
        .iter()
        .filter(|m| baseline.get(&(m.provider.clone(), m.id.clone())) != Some(m))
        .cloned()
        .collect();
    assert!(
        changes == expected.changes,
        "descriptor changes: {group} case {index}"
    );
    for (actual, expected) in changes.iter().zip(&expected.changes) {
        assert!(
            actual.compat.as_ref().map(|c| key_order(&c.0))
                == expected.compat.as_ref().map(|c| key_order(&c.0)),
            "compat key order case {index}"
        );
    }
    check_keys(&actual, expected, index);
    match &expected.error {
        None => assert_eq!(registry.get_error(), None, "{group} case {index}"),
        Some(error) => {
            let actual = registry.get_error().expect("load diagnostic");
            if let Some(full) = &error.full {
                assert_eq!(
                    actual,
                    full.replace("<models-file>", path),
                    "{group} case {index}"
                );
            }
            if let Some(kind) = &error.kind {
                assert!(
                    actual.starts_with(&format!("{kind}: ")),
                    "parser envelope case {index}"
                );
                assert!(
                    actual.ends_with(&format!("\n\nFile: {path}")),
                    "parser path case {index}"
                );
                assert!(actual.len() > kind.len() + path.len() + 10);
            }
        }
    }
}

use crate::descriptors::descriptors;

/// Compare nested compatibility-object key order independently of map equality.
fn key_order(object: &maestro_models::JsonObject) -> Vec<Vec<String>> {
    let mut order = vec![object.keys().cloned().collect()];
    for value in object.values() {
        if let serde_json::Value::Object(object) = value {
            order.extend(key_order(object));
        }
    }
    order
}
