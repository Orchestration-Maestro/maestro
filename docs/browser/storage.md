# Browser-native record storage

On WebAssembly, `maestro_web::IndexedDBStorageBackend` opens a browser database
lazily from the application's `IndexedDBConfig`. Calls share the retained opening
result, including a failed open. Upgrade creates configured stores and indices
only for names not already present; existing stores are left untouched.

`get`, `set` and `delete` each use the named collection's own transaction. Reads
are readonly; writes and deletes are readwrite. A successful operation means its
request succeeded, not that the transaction has committed. Missing or null reads
return `None`. Values remain native `JsValue` handles, including structured-clone
objects; this concrete API does not serialize them to JSON. Writes inspect the
live store key path to choose whether to supply the separate key.

```no_run
# #[cfg(target_arch = "wasm32")]
# async fn example() -> std::io::Result<()> {
use maestro_web::{IndexedDBConfig, IndexedDBStorageBackend, StoreConfig};
use wasm_bindgen::JsValue;

let database = IndexedDBStorageBackend::new(IndexedDBConfig {
    db_name: "workspace".into(),
    version: 1.0,
    stores: vec![StoreConfig {
        name: "records".into(),
        key_path: None,
        auto_increment: None,
        indices: None,
    }],
});
database.set("records", "greeting", JsValue::from_str("hello")).await?;
let greeting: Option<JsValue> = database.get("records", "greeting").await?;
database.delete("records", "greeting").await?;
# Ok(())
# }
```

Native policy tests cover store selection, explicit-key selection and failure
categories. The WebAssembly build checks platform bindings and the SDK signatures;
it does not prove browser persistence, upgrade, request teardown or rollback.
Real-browser qualification is tracked in [#1061](https://github.com/Orchestration-Maestro/maestro/issues/1061)
without a separate browser runner.

This concrete type does not yet implement the application's full
[`StorageBackend`](../application/browser-storage.md) contract. Enumeration,
index cursors, clear/has, multi-store transactions, quota/persistence and the
application-valued adapter are tracked in
[#1062](https://github.com/Orchestration-Maestro/maestro/issues/1062).
