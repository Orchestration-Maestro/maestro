//! JSON application faces share native operations and retained transactions.
//!
//! Native reads use the browser JSON representation: negative zero becomes zero,
//! nonfinite numbers become null, dates become strings and ordinary blobs/buffers
//! become objects. Cycles, BigInt and undefined JSON output are errors. Raw callers
//! retain unrestricted structured-clone values and never enter this conversion.
use super::{IndexedDBStorageBackend, IndexedDBStorageTransaction, browser_error};
use maestro_app::browser::storage::{IndexDirection, QuotaInfo, StorageOperation, TransactionMode};
use serde_json::Value;
use std::{future::Future, io, pin::Pin};
use wasm_bindgen::JsValue;

/// Borrowed application future without a Send requirement.
type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + 'a>>;
/// Convert a present native value through the platform's JSON representation.
fn decode_json(value: &JsValue) -> io::Result<Value> {
    let text = js_sys::JSON::stringify(value)
        .map_err(|error| browser_error(&error))?
        .as_string()
        .ok_or(io::ErrorKind::InvalidData)?;
    serde_json::from_str(&text).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
/// Parse an application JSON value without a handwritten native-value codec.
fn encode_json(value: &Value) -> io::Result<JsValue> {
    let text = serde_json::to_string(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    js_sys::JSON::parse(&text).map_err(|error| browser_error(&error))
}
impl maestro_app::StorageBackend for IndexedDBStorageBackend {
    fn get<'a>(&'a self, store_name: &'a str, key: &'a str) -> StorageFuture<'a, Option<Value>> {
        Box::pin(async move {
            IndexedDBStorageBackend::get(self, store_name, key)
                .await?
                .as_ref()
                .map(decode_json)
                .transpose()
        })
    }
    fn set<'a>(&'a self, store_name: &'a str, key: &'a str, value: Value) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            IndexedDBStorageBackend::set(self, store_name, key, encode_json(&value)?).await
        })
    }
    fn delete<'a>(&'a self, store_name: &'a str, key: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(IndexedDBStorageBackend::delete(self, store_name, key))
    }
    fn keys<'a>(
        &'a self,
        store_name: &'a str,
        prefix: Option<&'a str>,
    ) -> StorageFuture<'a, Vec<String>> {
        Box::pin(IndexedDBStorageBackend::keys(self, store_name, prefix))
    }
    fn get_all_from_index<'a>(
        &'a self,
        store_name: &'a str,
        index_name: &'a str,
        direction: IndexDirection,
    ) -> StorageFuture<'a, Vec<Value>> {
        Box::pin(async move {
            IndexedDBStorageBackend::get_all_from_index(self, store_name, index_name, direction)
                .await?
                .iter()
                .map(decode_json)
                .collect()
        })
    }
    fn clear<'a>(&'a self, store_name: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(IndexedDBStorageBackend::clear(self, store_name))
    }
    fn has<'a>(&'a self, store_name: &'a str, key: &'a str) -> StorageFuture<'a, bool> {
        Box::pin(IndexedDBStorageBackend::has(self, store_name, key))
    }
    fn transaction<'a>(
        &'a self,
        store_names: &'a [String],
        mode: TransactionMode,
        operation: StorageOperation<'a>,
    ) -> StorageFuture<'a, Value> {
        Box::pin(IndexedDBStorageBackend::transaction(
            self,
            store_names,
            mode,
            |tx| operation(tx),
        ))
    }
    fn get_quota_info(&self) -> StorageFuture<'_, QuotaInfo> {
        Box::pin(IndexedDBStorageBackend::get_quota_info(self))
    }
    fn request_persistence(&self) -> StorageFuture<'_, bool> {
        Box::pin(IndexedDBStorageBackend::request_persistence(self))
    }
}
impl maestro_app::StorageTransaction for IndexedDBStorageTransaction {
    fn get<'a>(&'a self, store_name: &'a str, key: &'a str) -> StorageFuture<'a, Option<Value>> {
        Box::pin(async move {
            IndexedDBStorageTransaction::get(self, store_name, key)
                .await?
                .as_ref()
                .map(decode_json)
                .transpose()
        })
    }
    fn set<'a>(&'a self, store_name: &'a str, key: &'a str, value: Value) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            IndexedDBStorageTransaction::set(self, store_name, key, encode_json(&value)?).await
        })
    }
    fn delete<'a>(&'a self, store_name: &'a str, key: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(IndexedDBStorageTransaction::delete(self, store_name, key))
    }
}
