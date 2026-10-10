#![cfg(test)]
//! Controlled files and source-derived expected catalog changes.
use maestro_catalog::{ModelFileOperations, ModelRegistry};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::{Model, get_models, get_providers};
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Mutable file text for reload scenarios.
#[derive(Clone)]
pub struct Text(pub Arc<Mutex<Option<String>>>);
impl ModelFileOperations for Text {
    fn exists(&self, _: &str) -> bool {
        self.0.lock().unwrap().is_some()
    }
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        self.0
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
    }
}
/// Empty stored credentials.
pub fn auth() -> Arc<AuthStorage> {
    Arc::new(AuthStorage::in_memory(AuthStorageData::new()))
}
/// Registry over controlled text.
pub fn registry(text: &str) -> ModelRegistry {
    ModelRegistry::with_operations(
        auth(),
        "<models-file>",
        Text(Arc::new(Mutex::new(Some(text.into())))),
    )
    .unwrap()
}
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
/// Disposable native file, created exclusively.
pub struct File(pub PathBuf);
impl File {
    pub fn new(bytes: &[u8]) -> Self {
        use std::io::Write;
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-catalog-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        Self(path)
    }
    pub fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
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
        let file = File::new(case.input.as_bytes());
        for (registry, path) in [
            (registry(&case.input), "<models-file>"),
            (
                ModelRegistry::create(auth(), file.path()).unwrap(),
                file.path(),
            ),
        ] {
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

/// Reject unread descriptor and cost fields before reusing the shared model decoder.
fn descriptors<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Vec<Model>, D::Error> {
    let values = Vec::<serde_json::Value>::deserialize(decoder)?;
    values
        .into_iter()
        .map(|value| {
            let model = value
                .as_object()
                .ok_or_else(|| serde::de::Error::custom("expected descriptor object"))?;
            if model.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "id" | "name"
                        | "api"
                        | "provider"
                        | "baseUrl"
                        | "reasoning"
                        | "thinkingLevelMap"
                        | "input"
                        | "cost"
                        | "contextWindow"
                        | "maxTokens"
                        | "headers"
                        | "compat"
                )
            }) {
                return Err(serde::de::Error::custom("unread descriptor field"));
            }
            if model
                .get("cost")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|cost| {
                    cost.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            "input" | "output" | "cacheRead" | "cacheWrite"
                        )
                    })
                })
            {
                return Err(serde::de::Error::custom("unread cost field"));
            }
            Model::deserialize(value).map_err(serde::de::Error::custom)
        })
        .collect()
}

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
