//! Controlled collection adapter and lossless corpus comparisons.
use futures_util::future::LocalBoxFuture;
use maestro_app::browser::storage::{
    IndexDirection, QuotaInfo, StorageBackend, StorageOperation, TransactionMode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, io, rc::Rc, sync::Arc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Recorded corpus envelope.
pub struct Corpus {
    /// Complete cases.
    pub rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// One source-recorded operation and result.
pub struct Row {
    /// Owning behavior test.
    pub test: String,
    /// Operation operands consumed by the test.
    pub input: Value,
    /// Complete observation.
    pub expected: Value,
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
            &["error", "trace"]
        } else {
            &["error"]
        };
        assert_eq!(
            expected.keys().map(String::as_str).collect::<Vec<_>>(),
            fields
        );
    }
    rows
}
/// Compare all members in order, keeping floating-point signs.
pub fn equal(actual: &Value, expected: &Value) {
    compare(actual, expected, "", false);
}
/// Compare typed provider/model record fields by name, preserving ordered maps.
pub fn provider_equal(actual: &Value, expected: &Value) {
    compare(actual, expected, "", true);
}
/// Compare typed record fields by name, while preserving ordered data.
fn compare(actual: &Value, expected: &Value, field: &str, records: bool) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            assert_eq!(a.as_f64().unwrap().to_bits(), b.as_f64().unwrap().to_bits());
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                compare(a, b, field, records);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            let typed = records
                && a.contains_key("baseUrl")
                && (a.contains_key("api") || a.contains_key("type"));
            assert_eq!(a.len(), b.len());
            if !typed || ["headers", "thinkingLevelMap", "compat"].contains(&field) {
                assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            }
            for (key, value) in a {
                compare(value, b.get(key).expect("record member"), key, records);
            }
        }
        _ => assert!(actual == expected, "recorded value differs"),
    }
}
/// A distinguishable source error, without logging stored values.
#[derive(Debug)]
pub struct Cause(pub Arc<()>);
impl std::fmt::Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("controlled adapter failure")
    }
}
impl std::error::Error for Cause {}
/// Supplied adapter with ordered observation and controllable suspension.
#[derive(Default)]
pub struct Backend {
    /// Stored collections, whose object maps preserve insertion order.
    pub data: RefCell<BTreeMap<String, serde_json::Map<String, Value>>>,
    /// Complete call transcript.
    pub trace: RefCell<Vec<Value>>,
    /// Supplied enumeration, independent of stored map order.
    pub keys: RefCell<Option<Vec<String>>>,
    /// Failing operation.
    pub fail: RefCell<Option<String>>,
    /// A particular failing read.
    pub fail_key: RefCell<Option<String>>,
    /// Original adapter cause identity.
    pub cause: Arc<()>,
    /// Next read completion gate.
    pub read_gate: RefCell<Option<tokio::sync::oneshot::Receiver<Value>>>,
    /// Next key enumeration completion gate.
    pub keys_gate: RefCell<Option<tokio::sync::oneshot::Receiver<Vec<String>>>>,
}
impl Backend {
    /// New isolated supplied adapter.
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
    /// Seed one collection without an observable adapter call.
    pub fn seed(&self, name: &str, values: Value) {
        let Value::Object(values) = values else {
            panic!("seed requires an object")
        };
        self.data.borrow_mut().insert(name.into(), values);
    }
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
    /// Record a reached adapter effect or return its original cause.
    fn effect(&self, op: &str, args: Vec<Value>) -> io::Result<()> {
        let fail_key = self.fail_key.borrow().clone();
        let fails = self.fail.borrow().as_deref() == Some(op)
            || (op == "get" && fail_key.as_deref().is_some_and(|key| args[1] == key));
        let mut call = vec![json!(op)];
        call.extend(args);
        self.trace.borrow_mut().push(Value::Array(call));
        if fails {
            Err(io::Error::other(Cause(self.cause.clone())))
        } else {
            Ok(())
        }
    }
}
impl StorageBackend for Backend {
    fn get<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<Option<Value>>> {
        Box::pin(async move {
            self.effect("get", vec![json!(s), json!(k)])?;
            let gate = self.read_gate.borrow_mut().take();
            let value = if let Some(gate) = gate {
                Some(gate.await.unwrap())
            } else {
                self.data.borrow().get(s).and_then(|m| m.get(k)).cloned()
            };
            Ok(value.filter(|v| !v.is_null()))
        })
    }
    fn set<'a>(&'a self, s: &'a str, k: &'a str, v: Value) -> LocalBoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.effect("set", vec![json!(s), json!(k), v.clone()])?;
            self.data
                .borrow_mut()
                .entry(s.into())
                .or_default()
                .insert(k.into(), v);
            Ok(())
        })
    }
    fn delete<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.effect("delete", vec![json!(s), json!(k)])?;
            self.data
                .borrow_mut()
                .entry(s.into())
                .or_default()
                .shift_remove(k);
            Ok(())
        })
    }
    fn keys<'a>(
        &'a self,
        s: &'a str,
        prefix: Option<&'a str>,
    ) -> LocalBoxFuture<'a, io::Result<Vec<String>>> {
        Box::pin(async move {
            assert!(prefix.is_none());
            self.effect("keys", vec![json!(s)])?;
            let gate = self.keys_gate.borrow_mut().take();
            if let Some(gate) = gate {
                return Ok(gate.await.unwrap());
            }
            Ok(self.keys.borrow().clone().unwrap_or_else(|| {
                self.data
                    .borrow()
                    .get(s)
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default()
            }))
        })
    }
    fn clear<'a>(&'a self, s: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.effect("clear", vec![json!(s)])?;
            self.data.borrow_mut().entry(s.into()).or_default().clear();
            Ok(())
        })
    }
    fn has<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<bool>> {
        Box::pin(async move {
            self.effect("has", vec![json!(s), json!(k)])?;
            Ok(self.data.borrow().get(s).is_some_and(|m| m.contains_key(k)))
        })
    }
    fn get_all_from_index<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        _: IndexDirection,
    ) -> LocalBoxFuture<'a, io::Result<Vec<Value>>> {
        unreachable!("not a named-store effect")
    }
    fn transaction<'a>(
        &'a self,
        _: &'a [String],
        _: TransactionMode,
        _: StorageOperation<'a>,
    ) -> LocalBoxFuture<'a, io::Result<Value>> {
        unreachable!("not a named-store effect")
    }
    fn get_quota_info(&self) -> LocalBoxFuture<'_, io::Result<QuotaInfo>> {
        unreachable!("not a named-store effect")
    }
    fn request_persistence(&self) -> LocalBoxFuture<'_, io::Result<bool>> {
        unreachable!("not a named-store effect")
    }
}
