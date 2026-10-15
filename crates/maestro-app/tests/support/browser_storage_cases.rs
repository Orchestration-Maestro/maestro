//! Collection corpus selection and whole-collection observations.
use super::support::{Backend, Row};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Recorded corpus envelope.
pub struct Corpus {
    /// Complete cases.
    pub rows: Vec<Row>,
}
/// Strict cases for a named test.
pub fn rows(name: &str) -> Vec<Row> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/browser_storage.json")).unwrap();
    assert_eq!(corpus.rows.len(), 88);
    let rows: Vec<_> = corpus.rows.into_iter().filter(|r| r.test == name).collect();
    assert!(!rows.is_empty());
    for row in &rows {
        let fields: &[&str] = match name {
            "store_schemas_keep_out_of_line_keys"
            | "schema_reads_return_independent_owned_records"
            | "store_binding_replaces_shared_backend_and_retains_inflight_operation" => &["kind"],
            "store_backend_required_for_each_operation"
            | "store_failures_keep_operation_phase_and_adapter_cause" => &["kind", "op"],
            "settings_values_roundtrip_without_normalization" => &["key", "value"],
            "settings_missing_delete_list_and_clear_use_only_settings" => &["key", "keys"],
            "provider_keys_keep_empty_and_unicode_secrets" => &["provider", "key"],
            "provider_key_overwrite_list_delete_and_presence_use_named_store" => &["providers"],
            "custom_provider_records_keep_types_optional_values_and_models" => &["provider"],
            "custom_provider_overwrite_delete_and_presence_use_named_store" => &["id"],
            "storage_config_records_preserve_optional_settings" => &["config"],
            _ => &["keys"],
        };
        assert_eq!(
            row.input
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            fields
        );
        let expected = row.expected.as_object().unwrap();
        let fields: &[&str] = if expected.contains_key("result") {
            &["result"]
        } else if expected.contains_key("trace") {
            &["trace"]
        } else {
            &[]
        };
        assert_eq!(
            expected.keys().map(String::as_str).collect::<Vec<_>>(),
            fields
        );
    }
    rows
}
impl Backend {
    /// Read a complete collection as ordered key/value pairs.
    pub fn entries(&self, name: &str) -> Value {
        Value::Array(
            self.data
                .borrow()
                .get(name)
                .into_iter()
                .flat_map(|m| m.iter())
                .map(|(k, v)| json!([k, v]))
                .collect(),
        )
    }
}
