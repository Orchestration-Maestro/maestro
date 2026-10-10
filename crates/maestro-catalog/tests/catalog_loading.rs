#![cfg(test)]
//! File loading through the public registry.
use maestro_catalog::{ModelFileOperations, ModelRegistry};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::{get_models, get_providers};
use std::sync::Arc;

/// Adapter that rejects any unexpected read.
struct Missing;
impl ModelFileOperations for Missing {
    fn exists(&self, path: &str) -> bool {
        assert!(!path.is_empty(), "empty paths must skip existence checks");
        false
    }
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        panic!("missing file was read")
    }
}
#[test]
fn memory_and_missing_files_use_complete_offline_catalog() {
    let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::new()));
    let expected: Vec<_> = get_providers().iter().flat_map(|p| get_models(p)).collect();
    assert_eq!(expected.len(), 970);
    for registry in [
        ModelRegistry::in_memory(auth.clone()).unwrap(),
        ModelRegistry::with_operations(auth.clone(), "absent", Missing).unwrap(),
        ModelRegistry::with_operations(auth.clone(), "", Missing).unwrap(),
    ] {
        assert!(Arc::ptr_eq(registry.auth_storage(), &auth));
        assert_eq!(registry.get_error(), None);
        let all = registry.get_all();
        let actual: Vec<_> = all
            .read()
            .unwrap()
            .iter()
            .map(|m| m.read().unwrap().clone())
            .collect();
        assert!(actual == expected, "complete ordered offline descriptors");
    }
}
mod support;
#[test]
fn schema_rejects_wrong_types_with_ordered_paths() {
    support::corpus("schema_rejects_wrong_types_with_ordered_paths");
}
#[test]
fn additional_validation_reports_first_source_order_failure() {
    support::corpus("additional_validation_reports_first_source_order_failure");
}
#[test]
fn json_text_stripping_preserves_strings_and_json_boundaries() {
    support::corpus("json_text_stripping_preserves_strings_and_json_boundaries");
}

#[test]
fn schema_accepts_declared_shapes_without_protocol_discrimination() {
    support::corpus("schema_accepts_declared_shapes_without_protocol_discrimination");
}

#[test]
fn loading_never_resolves_keys_or_headers() {
    support::corpus("loading_never_resolves_keys_or_headers");
    let marker = support::File::new(b"untouched");
    let command = format!("!printf changed > '{}'", marker.path());
    let input = serde_json::json!({"providers":{"custom":{"baseUrl":"u","api":"a","apiKey":command,"headers":{"X-Provider":command},"models":[{"id":"marker","headers":{"X-Model":command}}]}}}).to_string();
    let file = support::File::new(input.as_bytes());
    for registry in [
        support::registry(&input),
        ModelRegistry::create(support::auth(), file.path()).unwrap(),
    ] {
        assert_eq!(registry.get_error(), None);
        assert!(
            registry
                .find("custom", "marker")
                .unwrap()
                .read()
                .unwrap()
                .headers
                .is_none()
        );
        assert_eq!(std::fs::read(marker.path()).unwrap(), b"untouched");
    }
}

#[test]
fn schema_failure_precedes_additional_validation() {
    support::corpus("schema_failure_precedes_additional_validation");
}

#[test]
fn schema_diagnostics_keep_distinct_errors_in_source_order() {
    support::corpus("schema_diagnostics_keep_distinct_errors_in_source_order");
}
/// File disappears after existence is observed.
struct Disappearing(Arc<std::sync::atomic::AtomicBool>);
impl ModelFileOperations for Disappearing {
    fn exists(&self, _: &str) -> bool {
        true
    }
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        if self.0.load(std::sync::atomic::Ordering::SeqCst) {
            Err(std::io::ErrorKind::NotFound.into())
        } else {
            Ok(r#"{"providers":{}}"#.into())
        }
    }
}
#[test]
fn native_and_controlled_reads_share_failure_boundaries() {
    let initial = r#"{"providers":{}}"#;
    let file = support::File::new(initial.as_bytes());
    let text = support::Text(Arc::new(std::sync::Mutex::new(Some(initial.into()))));
    let mut native = ModelRegistry::create(support::auth(), file.path()).unwrap();
    let mut controlled =
        ModelRegistry::with_operations(support::auth(), "<models-file>", text.clone()).unwrap();
    std::fs::remove_file(file.path()).unwrap();
    *text.0.lock().unwrap() = None;
    for registry in [&mut native, &mut controlled] {
        registry.refresh().unwrap();
        assert_eq!(registry.get_error(), None);
    }
    std::fs::create_dir(file.path()).unwrap();
    native.refresh().unwrap();
    assert!(
        native
            .get_error()
            .unwrap()
            .starts_with("Failed to load models.json: ")
    );
    assert!(
        native
            .get_error()
            .unwrap()
            .ends_with(&format!("\n\nFile: {}", file.path()))
    );
    std::fs::remove_dir(file.path()).unwrap();
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut disappeared =
        ModelRegistry::with_operations(support::auth(), "gone", Disappearing(fail.clone()))
            .unwrap();
    assert!(
        disappeared
            .get_error()
            .unwrap()
            .starts_with("Failed to load models.json: ")
    );
    assert!(disappeared.get_error().unwrap().ends_with("\n\nFile: gone"));
    fail.store(false, std::sync::atomic::Ordering::SeqCst);
    disappeared.refresh().unwrap();
    assert_eq!(disappeared.get_error(), None);
    assert_eq!(disappeared.get_all().read().unwrap().len(), 970);
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(file.path())
        .unwrap();
    std::fs::write(file.path(), initial).unwrap();
    *text.0.lock().unwrap() = Some(initial.into());
    for registry in [&mut native, &mut controlled] {
        registry.refresh().unwrap();
        assert_eq!(registry.get_error(), None);
        assert_eq!(registry.get_all().read().unwrap().len(), 970);
    }
    native_replacement_decodes_utf8();
}
/// Check native replacement decoding before JSON admission.
fn native_replacement_decodes_utf8() {
    let mut invalid =
        r#"{"providers":{"custom":{"baseUrl":"u","apiKey":"unused","api":"a","models":[{"id":""#
            .as_bytes()
            .to_vec();
    invalid.push(255);
    invalid.extend_from_slice(b"\"}]}}}");
    let lossy = support::File::new(&invalid);
    let registry = ModelRegistry::create(support::auth(), lossy.path()).unwrap();
    assert!(registry.find("custom", "\u{fffd}").is_some());
    assert_eq!(registry.get_error(), None);
}
