//! Storage contracts exercised through browser and application SDK exports.
#![cfg(test)]
use maestro_web::{
    AutoDiscoveryProviderType, CustomProvider, CustomProviderType, CustomProvidersStore,
    IndexConfig, IndexDirection, IndexedDBConfig, ManualProviderType, ProviderKeysStore, QuotaInfo,
    SettingsStore, StorageBackend, StorageOperation, StorageTransaction, Store, StoreConfig,
    TransactionMode,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// Borrowed non-Send adapter future.
type Local<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + 'a>>;
/// Immediately-ready contract adapter; no platform algorithm is claimed.
#[derive(Default)]
struct Adapter {
    /// Complete consumed arguments.
    calls: RefCell<Vec<Value>>,
    /// One supplied key/value slot.
    value: RefCell<Option<Value>>,
}
impl StorageBackend for Adapter {
    fn get<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, Option<Value>> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["get", s, k]));
            Ok(self.value.borrow().clone())
        })
    }
    fn set<'a>(&'a self, s: &'a str, k: &'a str, v: Value) -> Local<'a, ()> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["set", s, k]));
            *self.value.borrow_mut() = Some(v);
            Ok(())
        })
    }
    fn delete<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, ()> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["delete", s, k]));
            *self.value.borrow_mut() = None;
            Ok(())
        })
    }
    fn keys<'a>(&'a self, s: &'a str, p: Option<&'a str>) -> Local<'a, Vec<String>> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["keys", s, p]));
            Ok(vec![p.unwrap_or("absent").into()])
        })
    }
    fn get_all_from_index<'a>(
        &'a self,
        s: &'a str,
        i: &'a str,
        d: IndexDirection,
    ) -> Local<'a, Vec<Value>> {
        Box::pin(async move {
            let d = match d {
                IndexDirection::Asc => "asc",
                IndexDirection::Desc => "desc",
            };
            self.calls.borrow_mut().push(json!(["index", s, i, d]));
            Ok(vec![json!(d)])
        })
    }
    fn clear<'a>(&'a self, s: &'a str) -> Local<'a, ()> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["clear", s]));
            *self.value.borrow_mut() = None;
            Ok(())
        })
    }
    fn has<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, bool> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["has", s, k]));
            Ok(self.value.borrow().is_some())
        })
    }
    fn transaction<'a>(
        &'a self,
        s: &'a [String],
        m: TransactionMode,
        op: StorageOperation<'a>,
    ) -> Local<'a, Value> {
        Box::pin(async move {
            let mode = match m {
                TransactionMode::Readonly => "readonly",
                TransactionMode::Readwrite => "readwrite",
            };
            self.calls
                .borrow_mut()
                .push(json!(["transaction", s, mode]));
            op(Rc::new(Self::default())).await
        })
    }
    fn get_quota_info(&self) -> Local<'_, QuotaInfo> {
        Box::pin(async {
            Ok(QuotaInfo {
                usage: 3.0,
                quota: 12.0,
                percent: 25.0,
            })
        })
    }
    fn request_persistence(&self) -> Local<'_, bool> {
        Box::pin(async { Ok(false) })
    }
}
impl StorageTransaction for Adapter {
    fn get<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, Option<Value>> {
        StorageBackend::get(self, s, k)
    }
    fn set<'a>(&'a self, s: &'a str, k: &'a str, v: Value) -> Local<'a, ()> {
        StorageBackend::set(self, s, k, v)
    }
    fn delete<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, ()> {
        StorageBackend::delete(self, s, k)
    }
}
/// Drive this adapter's immediately-ready futures without adding an executor.
fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("contract adapter unexpectedly suspended"),
    }
}
#[test]
/// Browser storage exports use application owners.
fn browser_storage_exports_use_application_owners() {
    ready(async {
        let settings: maestro_app::SettingsStore = SettingsStore::default();
        let _: maestro_app::ProviderKeysStore = ProviderKeysStore::default();
        let _: maestro_app::CustomProvidersStore = CustomProvidersStore::default();
        let index: maestro_app::IndexConfig = IndexConfig {
            name: "recent".into(),
            key_path: "time".into(),
            unique: Some(true),
        };
        let config: maestro_app::StoreConfig = StoreConfig {
            name: "one".into(),
            key_path: Some("id".into()),
            auto_increment: Some(false),
            indices: Some(vec![index]),
        };
        let db: maestro_app::IndexedDBConfig = IndexedDBConfig {
            db_name: "db".into(),
            version: 2.0,
            stores: vec![config.clone()],
        };
        let base: maestro_app::Store = Store::new(config.clone());
        assert_eq!(base.get_config(), db.stores[0]);
        let b = Rc::new(Adapter::default());
        let backend: Rc<dyn maestro_app::StorageBackend> = b.clone();
        settings.set_backend(backend.clone());
        settings
            .set("layout", &json!(["wide", false]))
            .await
            .unwrap();
        assert_eq!(
            settings.get::<Value>("layout").await.unwrap(),
            Some(json!(["wide", false]))
        );
        exported_provider_records();
        exercise_contract(backend.as_ref()).await;
        assert_eq!(
            *b.calls.borrow(),
            json!([
                ["set", "settings", "layout"],
                ["get", "settings", "layout"],
                ["keys", "one", null],
                ["keys", "one", ""],
                ["keys", "one", "pre"],
                ["index", "one", "recent", "asc"],
                ["index", "one", "recent", "desc"],
                ["transaction", ["one", "two"], "readonly"],
                ["transaction", ["one", "two"], "readwrite"],
                ["has", "one", "k"],
                ["delete", "one", "k"],
                ["clear", "one"]
            ])
            .as_array()
            .unwrap()
            .clone()
        );
    });
}
/// Pass every contract operand through a dyn backend and dyn transaction.
async fn exercise_contract(backend: &dyn StorageBackend) {
    exercise_enumeration(backend).await;
    exercise_transactions(backend).await;
    assert!(backend.has("one", "k").await.unwrap());
    backend.delete("one", "k").await.unwrap();
    backend.clear("one").await.unwrap();
    assert_eq!(
        backend.get_quota_info().await.unwrap(),
        QuotaInfo {
            usage: 3.0,
            quota: 12.0,
            percent: 25.0
        }
    );
    assert!(!backend.request_persistence().await.unwrap());
}

/// Construct all exported provider families and protocols.
fn exported_provider_records() {
    let families: [maestro_app::AutoDiscoveryProviderType; 4] = [
        AutoDiscoveryProviderType::Ollama,
        AutoDiscoveryProviderType::LlamaCpp,
        AutoDiscoveryProviderType::Vllm,
        AutoDiscoveryProviderType::Lmstudio,
    ];
    let manuals: [maestro_app::ManualProviderType; 3] = [
        ManualProviderType::OpenaiCompletions,
        ManualProviderType::OpenaiResponses,
        ManualProviderType::AnthropicMessages,
    ];
    let kinds: Vec<maestro_app::CustomProviderType> = families
        .into_iter()
        .map(CustomProviderType::AutoDiscovery)
        .chain(manuals.into_iter().map(CustomProviderType::Manual))
        .collect();
    for (kind, expected) in kinds.into_iter().zip([
        "ollama",
        "llama.cpp",
        "vllm",
        "lmstudio",
        "openai-completions",
        "openai-responses",
        "anthropic-messages",
    ]) {
        let p: maestro_app::CustomProvider = CustomProvider {
            id: "p".into(),
            name: "P".into(),
            r#type: kind,
            base_url: "http://localhost".into(),
            api_key: maestro_app::Presence::Null,
            models: maestro_app::Presence::Missing,
        };
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            json!({"id":"p","name":"P","type":expected,"baseUrl":"http://localhost","apiKey":null})
        );
    }
}

/// Carry callback results through both transaction modes.
async fn exercise_transactions(backend: &dyn StorageBackend) {
    let stores = vec!["one".into(), "two".into()];
    for mode in [TransactionMode::Readonly, TransactionMode::Readwrite] {
        let result = backend
            .transaction(
                &stores,
                mode,
                Box::new(|tx: Rc<dyn maestro_app::StorageTransaction>| {
                    Box::pin(async move {
                        tx.set("two", "k", json!({"callback":17})).await?;
                        let result = tx.get("two", "k").await?;
                        tx.delete("two", "k").await?;
                        assert!(tx.get("two", "k").await?.is_none());
                        Ok(result.unwrap())
                    })
                }),
            )
            .await
            .unwrap();
        assert_eq!(result, json!({"callback":17}));
    }
}

/// Keep absent and empty prefixes distinct and expose both index directions.
async fn exercise_enumeration(backend: &dyn StorageBackend) {
    for prefix in [None, Some(""), Some("pre")] {
        assert_eq!(
            backend.keys("one", prefix).await.unwrap(),
            vec![prefix.unwrap_or("absent")]
        );
    }
    assert_eq!(IndexDirection::default(), IndexDirection::Asc);
    for (direction, expected) in [(IndexDirection::Asc, "asc"), (IndexDirection::Desc, "desc")] {
        assert_eq!(
            backend
                .get_all_from_index("one", "recent", direction)
                .await
                .unwrap(),
            vec![json!(expected)]
        );
    }
}
