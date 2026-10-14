//! Recorded availability and model-choice failures.
use maestro_catalog::{CatalogError, ModelRegistry};
use maestro_models::Model;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

/// One recorded public operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub test: String,
    pub input: Input,
    pub expected: Value,
}
/// Supplied operation inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub operation: Option<String>,
    pub configured: Option<bool>,
    pub fallback: Option<String>,
    pub failure_index: Option<usize>,
    pub models: Option<Vec<Descriptor>>,
    pub include_config: Option<bool>,
    pub patterns: Option<Vec<String>>,
    pub chosen: Option<String>,
    pub branch: Option<String>,
}
/// Recorded descriptor identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub provider: String,
    pub id: String,
    pub name: String,
}
/// Rows owned by the requested test.
pub fn cases(test: &str) -> Vec<Case> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/fallback_failures.json")).unwrap();
    let rows: Vec<_> = rows.into_iter().filter(|row| row.test == test).collect();
    assert!(!rows.is_empty());
    rows
}
/// Real registry whose single live descriptor has the recorded identity.
pub fn registry(configured: bool) -> ModelRegistry {
    let registry = crate::support::registry(
        r#"{"providers":{"maestro-custom-fallback":{"api":"test","baseUrl":"url","apiKey":"configured","models":[{"id":"m","name":"Model m"}]}}}"#,
    );
    let model = registry.find("maestro-custom-fallback", "m").unwrap();
    if configured {
        *registry.get_all().write().unwrap() = vec![model];
        registry
    } else {
        let registry = ModelRegistry::in_memory(crate::support::auth()).unwrap();
        *registry.get_all().write().unwrap() = vec![model];
        registry
    }
}
/// Descriptor made from existing metadata, replacing exactly the recorded fields.
pub fn descriptor(registry: &ModelRegistry, input: &Descriptor) -> Arc<RwLock<Model>> {
    let mut model = registry.get_all().read().unwrap()[0]
        .read()
        .unwrap()
        .clone();
    model.provider.clone_from(&input.provider);
    model.id.clone_from(&input.id);
    model.name.clone_from(&input.name);
    Arc::new(RwLock::new(model))
}
/// Project only the recorded descriptor fields, preserving identity and order.
pub fn model_value(model: &Arc<RwLock<Model>>) -> Value {
    let model = model.read().unwrap();
    json!({"provider":model.provider, "id":model.id, "name":model.name})
}
/// Install an ordinary failing callback and retain exact ordered invocations.
pub fn failing(registry: &ModelRegistry) -> Arc<Mutex<Vec<String>>> {
    crate::credential_fallback::install(registry.auth_storage(), "error")
}
/// Preserve the original account diagnostic through the catalog boundary.
pub fn error_value(error: &CatalogError) -> Value {
    let CatalogError::OAuth(error) = error else {
        panic!("expected original account failure")
    };
    crate::credential_fallback::error_value(error)
}
/// Whole result plus the callback transcript.
pub fn outcome<T>(result: Result<T, CatalogError>, calls: &Arc<Mutex<Vec<String>>>) -> Value {
    let error = result.err().expect("expected lookup failure");
    json!({"branch":"error", "error":error_value(&error), "calls":*calls.lock().unwrap()})
}
