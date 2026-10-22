//! Browser collection behavior through application-owned stores.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::browser_storage_support as support;
use crate::support::run_io;
use maestro_app::{
    CustomProvidersStore, IndexedDBConfig, ProviderKeysStore, SettingsStore, Store, StoreConfig,
};
use serde_json::{Value, json};
use std::{io, rc::Rc};
#[path = "support/browser_storage_cases.rs"]
mod cases;
use cases::rows;
use support::{Backend, equal, provider_equal};

/// A named store using the same public operations as SDK consumers.
enum Named {
    /// Settings collection.
    Settings(SettingsStore),
    /// Provider-key collection.
    Keys(ProviderKeysStore),
    /// Custom-provider collection.
    Providers(CustomProvidersStore),
}
impl Named {
    /// Construct the declared collection.
    fn new(kind: &str) -> Self {
        match kind {
            "SettingsStore" => Self::Settings(SettingsStore::default()),
            "ProviderKeysStore" => Self::Keys(ProviderKeysStore::default()),
            "CustomProvidersStore" => Self::Providers(CustomProvidersStore::default()),
            _ => panic!("unknown store"),
        }
    }
    /// Shared base access.
    fn base(&self) -> &Store {
        match self {
            Self::Settings(s) => s,
            Self::Keys(s) => s,
            Self::Providers(s) => s,
        }
    }
}
#[test]
/// Store schemas keep out of line keys.
fn store_schemas_keep_out_of_line_keys() {
    for row in rows("store_schemas_keep_out_of_line_keys") {
        let kind = row.input["kind"].as_str().unwrap();
        assert_eq!(row.input.as_object().unwrap().len(), 1);
        equal(
            &serde_json::to_value(Named::new(kind).base().get_config()).unwrap(),
            &row.expected["result"],
        );
    }
}
#[test]
/// Schema reads return independent owned records.
fn schema_reads_return_independent_owned_records() {
    for row in rows("schema_reads_return_independent_owned_records") {
        let store = Named::new(row.input["kind"].as_str().unwrap());
        let mut first = store.base().get_config();
        first.name = "mutated".into();
        equal(
            &serde_json::to_value(store.base().get_config()).unwrap(),
            &row.expected["result"],
        );
    }
    let config: StoreConfig = serde_json::from_value(json!({"name":"complete","keyPath":"id","autoIncrement":false,"indices":[{"name":"recent","keyPath":"time","unique":true}]})).unwrap();
    let store = Store::new(config.clone());
    let mut snapshot = store.get_config();
    snapshot.indices.as_mut().unwrap().clear();
    assert_eq!(store.get_config(), config);
}
#[test]
/// Store backend required for each operation.
fn store_backend_required_for_each_operation() {
    run_io(async {
        for row in rows("store_backend_required_for_each_operation") {
            let store = Named::new(row.input["kind"].as_str().unwrap());
            let error = store
                .invoke(row.input["op"].as_str().unwrap())
                .await
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::NotConnected);
            assert!(!store.base().get_config().name.is_empty());
        }
    });
}
#[test]
/// Settings values roundtrip without normalization.
fn settings_values_roundtrip_without_normalization() {
    run_io(async {
        for row in rows("settings_values_roundtrip_without_normalization") {
            let s = SettingsStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            let key = row.input["key"].as_str().unwrap();
            s.set(key, &row.input["value"]).await.unwrap();
            equal(
                &json!({"value":s.get::<Value>(key).await.unwrap(),"trace":*b.trace.borrow()}),
                &row.expected["result"],
            );
        }
    });
}

#[test]
/// Settings missing delete list and clear use only settings.
fn settings_missing_delete_list_and_clear_use_only_settings() {
    run_io(async {
        for row in rows("settings_missing_delete_list_and_clear_use_only_settings") {
            let s = SettingsStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            b.seed("settings", json!({"z":1,"a":2,"é":3}));
            b.seed("provider-keys", json!({"keep":"secret"}));
            *b.keys.borrow_mut() = Some(serde_json::from_value(row.input["keys"].clone()).unwrap());
            let key = row.input["key"].as_str().unwrap();
            let missing = s.get::<Value>(key).await.unwrap();
            let keys = s.list().await.unwrap();
            s.delete(key).await.unwrap();
            s.clear().await.unwrap();
            equal(
                &json!({"missing":missing,"keys":keys,"trace":*b.trace.borrow(),"settings":b.entries("settings"),"other":b.entries("provider-keys")}),
                &row.expected["result"],
            );
        }
    });
}

#[test]
/// Storage config records preserve optional settings.
fn storage_config_records_preserve_optional_settings() {
    run_io(async {
        for row in rows("storage_config_records_preserve_optional_settings") {
            let config: IndexedDBConfig =
                serde_json::from_value(row.input["config"].clone()).unwrap();
            let s = SettingsStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            s.set("schema", &config).await.unwrap();
            let result = s.get::<IndexedDBConfig>("schema").await.unwrap();
            equal(
                &json!({"value":result,"trace":*b.trace.borrow()}),
                &row.expected["result"],
            );
        }
    });
}
#[test]
/// Provider keys keep empty and unicode secrets.
fn provider_keys_keep_empty_and_unicode_secrets() {
    run_io(async {
        for row in rows("provider_keys_keep_empty_and_unicode_secrets") {
            let s = ProviderKeysStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            let provider = row.input["provider"].as_str().unwrap();
            let key = row.input["key"].as_str().unwrap();
            s.set(provider, key).await.unwrap();
            equal(
                &json!({"value":s.get(provider).await.unwrap(),"has":s.has(provider).await.unwrap(),"trace":*b.trace.borrow()}),
                &row.expected["result"],
            );
        }
    });
}

#[test]
/// Provider key overwrite list delete and presence use named store.
fn provider_key_overwrite_list_delete_and_presence_use_named_store() {
    run_io(async {
        for row in rows("provider_key_overwrite_list_delete_and_presence_use_named_store") {
            let ids: Vec<String> = serde_json::from_value(row.input["providers"].clone()).unwrap();
            let s = ProviderKeysStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            b.seed("settings", json!({"a":"keep"}));
            b.seed(
                "provider-keys",
                json!({ids[0].clone():"old",ids[1].clone():""}),
            );
            *b.keys.borrow_mut() = Some(ids.clone());
            let before = s.get("missing").await.unwrap();
            let absent = s.has("missing").await.unwrap();
            let empty = s.has(&ids[1]).await.unwrap();
            s.set(&ids[0], "new").await.unwrap();
            let overwrite = s.get(&ids[0]).await.unwrap();
            let keys = s.list().await.unwrap();
            s.delete(&ids[0]).await.unwrap();
            s.delete("missing").await.unwrap();
            let after = s.has(&ids[0]).await.unwrap();
            equal(
                &json!({"before":before,"absent":absent,"empty":empty,"overwrite":overwrite,"keys":keys,"after":after,"trace":*b.trace.borrow(),"other":b.entries("settings")}),
                &row.expected["result"],
            );
        }
    });
}

#[test]
/// Custom provider records keep types optional values and models.
fn custom_provider_records_keep_types_optional_values_and_models() {
    run_io(async {
        for row in rows("custom_provider_records_keep_types_optional_values_and_models") {
            let p: maestro_app::CustomProvider =
                serde_json::from_value(row.input["provider"].clone()).unwrap();
            let s = CustomProvidersStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            s.set(&p).await.unwrap();
            provider_equal(
                &json!({"value":s.get(&p.id).await.unwrap(),"has":s.has(&p.id).await.unwrap(),"trace":*b.trace.borrow()}),
                &row.expected["result"],
            );
        }
    });
}

#[test]
/// Thinking maps retain insertion order through reads, writes and listing.
fn custom_provider_thinking_maps_keep_insertion_order() {
    run_io(async {
        let row = rows("custom_provider_records_keep_types_optional_values_and_models")
            .into_iter()
            .find(|row| {
                row.input["provider"]["models"]
                    .as_array()
                    .is_some_and(|m| !m.is_empty())
            })
            .unwrap();
        let mut value = row.input["provider"].clone();
        value["models"][0]["thinkingLevelMap"] = json!({"high":"large","low":"small"});
        let s = CustomProvidersStore::default();
        let b = Backend::new();
        s.set_backend(b.clone());
        let id = value["id"].as_str().unwrap();
        b.seed("custom-providers", json!({id:value.clone()}));
        let read = serde_json::to_value(s.get(id).await.unwrap().unwrap()).unwrap();
        let p = serde_json::from_value::<maestro_app::CustomProvider>(value.clone()).unwrap();
        s.set(&p).await.unwrap();
        let stored = b.entries("custom-providers")[0][1].clone();
        let listed = serde_json::to_value(s.get_all().await.unwrap()).unwrap();
        for (phase, value) in [
            ("get", read),
            ("set", stored),
            ("get_all", listed[0].clone()),
        ] {
            assert_eq!(
                value["models"][0]["thinkingLevelMap"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["high", "low"],
                "{phase}"
            );
        }
    });
}

#[test]
/// Custom provider overwrite delete and presence use named store.
fn custom_provider_overwrite_delete_and_presence_use_named_store() {
    run_io(async {
        for row in rows("custom_provider_overwrite_delete_and_presence_use_named_store") {
            let id = row.input["id"].as_str().unwrap();
            let s = CustomProvidersStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            b.seed("settings", json!({"keep":1}));
            let mut p: maestro_app::CustomProvider = serde_json::from_value(
                json!({"id":id,"name":"old","type":"ollama","baseUrl":"http://localhost"}),
            )
            .unwrap();
            let missing = s.get(id).await.unwrap();
            let absent = s.has(id).await.unwrap();
            s.set(&p).await.unwrap();
            p.name = "new".into();
            s.set(&p).await.unwrap();
            let overwrite = s.get(id).await.unwrap();
            s.delete(id).await.unwrap();
            s.delete("gone").await.unwrap();
            let after = s.has(id).await.unwrap();
            equal(
                &json!({"missing":missing,"absent":absent,"overwrite":overwrite,"after":after,"trace":*b.trace.borrow(),"other":b.entries("settings")}),
                &row.expected["result"],
            );
        }
    });
}

/// Full provider with a distinguishable identifier and name.
fn provider(id: &str, name: &str) -> Value {
    json!({"id":id,"name":name,"type":"ollama","baseUrl":"http://localhost"})
}
#[test]
/// Custom provider listing keeps key order and skips removed rows.
fn custom_provider_listing_keeps_key_order_and_skips_removed_rows() {
    run_io(async {
        for row in rows("custom_provider_listing_keeps_key_order_and_skips_removed_rows") {
            let s = CustomProvidersStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            *b.keys.borrow_mut() = Some(serde_json::from_value(row.input["keys"].clone()).unwrap());
            b.seed("custom-providers",json!({"first":provider("first","first"),"second":provider("second","second"),"third":provider("third","third")}));
            equal(
                &json!({"providers":s.get_all().await.unwrap(),"trace":*b.trace.borrow()}),
                &row.expected["result"],
            );
        }
    });
}

impl Named {
    /// Invoke the source operation using its declared operands.
    async fn invoke(&self, op: &str) -> io::Result<Value> {
        match self {
            Self::Settings(s) => settings_operation(s, op).await,
            Self::Keys(s) => key_operation(s, op).await,
            Self::Providers(s) => provider_operation(s, op).await,
        }
    }
    /// Read a setting/key/descriptor via the public method.
    async fn read(&self) -> io::Result<Value> {
        self.invoke("get").await
    }
}
/// Assert the original error object survived delegation.
fn same_cause(error: &io::Error, b: &Backend) {
    let cause = error
        .get_ref()
        .unwrap()
        .downcast_ref::<support::Cause>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&cause.0, &b.cause));
}
#[test]
/// Store failures keep operation phase and adapter cause.
fn store_failures_keep_operation_phase_and_adapter_cause() {
    run_io(async {
        for row in rows("store_failures_keep_operation_phase_and_adapter_cause") {
            let s = Named::new(row.input["kind"].as_str().unwrap());
            let b = Backend::new();
            s.base().set_backend(b.clone());
            let op = row.input["op"].as_str().unwrap();
            let raw = if ["list", "getAll"].contains(&op) {
                "keys"
            } else {
                op
            };
            *b.fail.borrow_mut() = Some(raw.into());
            let error = s.invoke(op).await.unwrap_err();
            same_cause(&error, &b);
            equal(&json!({"trace":*b.trace.borrow()}), &row.expected);
        }
    });
}
#[test]
/// Custom provider listing stops at first read failure.
fn custom_provider_listing_stops_at_first_read_failure() {
    run_io(async {
        for row in rows("custom_provider_listing_stops_at_first_read_failure") {
            let s = CustomProvidersStore::default();
            let b = Backend::new();
            s.set_backend(b.clone());
            let keys: Vec<String> = serde_json::from_value(row.input["keys"].clone()).unwrap();
            b.seed(
                "custom-providers",
                json!({keys[0].clone():provider(&keys[0],&keys[0])}),
            );
            *b.fail_key.borrow_mut() = Some(keys[1].clone());
            *b.keys.borrow_mut() = Some(keys);
            let error = s.get_all().await.unwrap_err();
            same_cause(&error, &b);
            equal(&json!({"trace":*b.trace.borrow()}), &row.expected);
        }
    });
}
#[test]
/// Store binding replaces shared backend and retains inflight operation.
fn store_binding_replaces_shared_backend_and_retains_inflight_operation() {
    run_io(async {
        use futures_util::FutureExt;
        for row in rows("store_binding_replaces_shared_backend_and_retains_inflight_operation") {
            let s = Rc::new(Named::new(row.input["kind"].as_str().unwrap()));
            let alias = s.clone();
            let old = Backend::new();
            let new = Backend::new();
            let old_value = if matches!(&*s, Named::Providers(_)) {
                provider("k", "old")
            } else {
                json!("old")
            };
            let new_value = if matches!(&*s, Named::Providers(_)) {
                provider("k", "new")
            } else {
                json!("new")
            };
            let (release, gate) = tokio::sync::oneshot::channel();
            *old.read_gate.borrow_mut() = Some(gate);
            new.seed(&s.base().get_config().name, json!({"k":new_value}));
            s.base().set_backend(old.clone());
            let retained = s.base().get_backend().unwrap();
            assert!(Rc::ptr_eq(
                &retained,
                &(old.clone() as Rc<dyn maestro_app::StorageBackend>)
            ));
            let mut pending = Box::pin(alias.read());
            assert!(pending.as_mut().now_or_never().is_none());
            s.base().set_backend(new.clone());
            assert!(Rc::ptr_eq(
                &s.base().get_backend().unwrap(),
                &(new.clone() as Rc<dyn maestro_app::StorageBackend>)
            ));
            let latest = alias.read().await.unwrap();
            release.send(old_value).unwrap();
            let result = pending.await.unwrap();
            equal(
                &json!({"pending":result,"latest":latest,"oldTrace":*old.trace.borrow(),"newTrace":*new.trace.borrow()}),
                &row.expected["result"],
            );
            assert!(Rc::ptr_eq(
                &retained,
                &(old as Rc<dyn maestro_app::StorageBackend>)
            ));
        }
    });
}
#[test]
/// Custom provider listing resolves backend for each read.
fn custom_provider_listing_resolves_backend_for_each_read() {
    run_io(async {
        use futures_util::FutureExt;
        for row in rows("custom_provider_listing_resolves_backend_for_each_read") {
            let keys: Vec<String> = serde_json::from_value(row.input["keys"].clone()).unwrap();
            let s = CustomProvidersStore::default();
            let a = Backend::new();
            let b = Backend::new();
            let c = Backend::new();
            let (release_keys, keys_gate) = tokio::sync::oneshot::channel();
            *a.keys_gate.borrow_mut() = Some(keys_gate);
            let (release_first, first_gate) = tokio::sync::oneshot::channel();
            *b.read_gate.borrow_mut() = Some(first_gate);
            c.seed(
                "custom-providers",
                json!({keys[1].clone():provider(&keys[1],"last backend")}),
            );
            s.set_backend(a.clone());
            let mut pending = Box::pin(s.get_all());
            assert!(pending.as_mut().now_or_never().is_none());
            s.set_backend(b.clone());
            release_keys.send(keys.clone()).unwrap();
            assert!(pending.as_mut().now_or_never().is_none());
            assert_eq!(b.trace.borrow().len(), 1);
            s.set_backend(c.clone());
            release_first
                .send(provider(&keys[0], "middle backend"))
                .unwrap();
            equal(
                &json!({"providers":pending.await.unwrap(),"traces":[*a.trace.borrow(),*b.trace.borrow(),*c.trace.borrow()]}),
                &row.expected["result"],
            );
        }
    });
}

/// Invoke one settings operation at its public boundary.
async fn settings_operation(s: &SettingsStore, op: &str) -> io::Result<Value> {
    match op {
        "get" => Ok(json!(s.get::<Value>("k").await?)),
        "set" => {
            s.set("k", &"v").await?;
            Ok(Value::Null)
        }
        "delete" => {
            s.delete("k").await?;
            Ok(Value::Null)
        }
        "list" => Ok(json!(s.list().await?)),
        "clear" => {
            s.clear().await?;
            Ok(Value::Null)
        }
        _ => panic!("unknown operation"),
    }
}

/// Invoke one keys operation at its public boundary.
async fn key_operation(s: &ProviderKeysStore, op: &str) -> io::Result<Value> {
    match op {
        "get" => Ok(json!(s.get("k").await?)),
        "set" => {
            s.set("k", "v").await?;
            Ok(Value::Null)
        }
        "delete" => {
            s.delete("k").await?;
            Ok(Value::Null)
        }
        "list" => Ok(json!(s.list().await?)),
        "has" => Ok(json!(s.has("k").await?)),
        _ => panic!("unknown operation"),
    }
}

/// Invoke one providers operation at its public boundary.
async fn provider_operation(s: &CustomProvidersStore, op: &str) -> io::Result<Value> {
    match op {
        "get" => Ok(json!(s.get("k").await?)),
        "set" => {
            let p = serde_json::from_value(provider("p", "P")).unwrap();
            s.set(&p).await?;
            Ok(Value::Null)
        }
        "delete" => {
            s.delete("k").await?;
            Ok(Value::Null)
        }
        "getAll" => Ok(json!(s.get_all().await?)),
        "has" => Ok(json!(s.has("k").await?)),
        _ => panic!("unknown operation"),
    }
}
