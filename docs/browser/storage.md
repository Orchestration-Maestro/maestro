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

Dropping the final request owner settles its bridge before detaching callbacks.
Dropping an opening waiter does not settle or detach the backend's retained request.

Native tests cover store selection, explicit-key selection, prefix bounds, cursor
direction, quota arithmetic, failure categories and teardown ordering through the platform-independent ownership seam.
The WebAssembly build checks platform bindings and the SDK signatures;
it does not prove browser persistence, upgrade, request teardown or rollback.
Real-browser qualification is tracked in [#1061](https://github.com/Orchestration-Maestro/maestro/issues/1061)
without a separate browser runner.

The complete [`StorageBackend`](../application/browser-storage.md) implementation
adapts JSON application values on this same owner. Use `Rc<dyn StorageBackend>`
with application stores; the concrete API remains raw and unrestricted.

`keys` uses an inclusive range from a nonempty prefix to that prefix followed by
U+FFFF. An absent or empty prefix enumerates all keys. Key spelling and order
come from the browser, without trimming or sorting. Index cursors traverse all
values, including ties, in ascending or descending order. `has` uses key presence,
so a stored null record is still present; `clear` affects only its named store.

`transaction` supplies an owned
`storage::backends::indexeddb_storage_backend::IndexedDBStorageTransaction`.
Its get/set/delete methods use one retained native transaction for all supplied
stores in the selected readonly/readwrite mode. Returning the callback result
does not wait for commit, automatically abort on callback error, retry inactivity
or guarantee rollback. A retained handle does not keep a transaction active.

Estimation and persistence probe their own support independently and never open
the database. Unsupported estimation returns zero usage, quota and percent;
missing usage/quota independently default to zero. Percent is `(usage / quota) *
100`, or zero when quota is zero. Unsupported persistence returns false; supported
requests return the browser's grant decision. Promise rejections remain errors.

The JSON trait face uses platform JSON parse/stringify: native negative zero
becomes zero, nonfinite numbers become null, Date becomes its JSON string, and
ordinary Blob/buffer objects use their standard JSON object representation.
Cycles, `BigInt` and present values whose JSON representation is undefined fail;
missing raw reads stay `None`. Callers needing native types, numbers or identity
use the raw face. Application serialization policy itself is unchanged.

```no_run
# #[cfg(target_arch = "wasm32")]
# async fn application_example() -> std::io::Result<()> {
use maestro_web::{IndexedDBConfig, IndexedDBStorageBackend, StorageBackend,
    StoreConfig, TransactionMode};
use std::rc::Rc;
let raw = Rc::new(IndexedDBStorageBackend::new(IndexedDBConfig {
    db_name: "application".into(), version: 1.0,
    stores: vec![StoreConfig { name: "records".into(), key_path: None,
        auto_increment: None, indices: None }],
}));
let stores = vec!["records".into()];
let retained = raw.transaction(&stores, TransactionMode::Readwrite, |tx| async move {
    tx.set("records", "key", wasm_bindgen::JsValue::from_str("value")).await?;
    Ok(tx)
}).await?;
let application: Rc<dyn StorageBackend> = raw;
let keys = application.keys("records", None).await?;
let quota = application.get_quota_info().await?;
let granted = application.request_persistence().await?;
# Ok(())
# }
```
