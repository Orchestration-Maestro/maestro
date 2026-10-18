//! JSON application faces share native operations and retained transactions.
//!
//! Reads preserve JSON values, including negative zero. Values outside that
//! domain are errors. Raw callers retain unrestricted structured-clone values
//! and never enter this conversion.
use super::{IndexedDBStorageBackend, IndexedDBStorageTransaction, browser_error};
use crate::browser::json::encode_json;
use maestro_app::browser::storage::{IndexDirection, QuotaInfo, StorageOperation, TransactionMode};
use serde_json::Value;
use std::{future::Future, io, pin::Pin};
use wasm_bindgen::{JsCast, JsValue};

/// Borrowed application future without a Send requirement.
type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + 'a>>;
/// Convert a present JSON value without stringification or numeric normalization.
fn decode_json(value: &JsValue) -> io::Result<Value> {
    decode_value(value, &mut Vec::new())
}
/// Walk JSON values, rejecting cycles only along the current ancestry path.
fn decode_value(value: &JsValue, ancestors: &mut Vec<JsValue>) -> io::Result<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    if let Some(value) = value.as_bool() {
        return Ok(Value::Bool(value));
    }
    if let Some(value) = value.as_f64() {
        return super::super::json_number(value);
    }
    if let Some(value) = value.as_string() {
        return Ok(Value::String(value));
    }
    if !value.is_object()
        || ancestors
            .iter()
            .any(|ancestor| js_sys::Object::is(ancestor, value))
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    ancestors.push(value.clone());
    let result = decode_container(value, ancestors);
    ancestors.pop();
    result
}
/// Read arrays or enumerable string members of plain objects without coercion.
fn decode_container(value: &JsValue, ancestors: &mut Vec<JsValue>) -> io::Result<Value> {
    if js_sys::Array::is_array(value) {
        return js_sys::Array::from(value)
            .iter()
            .map(|value| decode_value(&value, ancestors))
            .collect::<io::Result<Vec<_>>>()
            .map(Value::Array);
    }
    let object: &js_sys::Object = value.unchecked_ref();
    let prototype = js_sys::Object::get_prototype_of(object);
    let plain_prototype = js_sys::Object::get_prototype_of(&js_sys::Object::new());
    if !prototype.is_null() && !js_sys::Object::is(&prototype, &plain_prototype) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    js_sys::Object::keys(object)
        .iter()
        .map(|key| {
            let name = key.as_string().ok_or(io::ErrorKind::InvalidData)?;
            let member =
                js_sys::Reflect::get(value, &key).map_err(|error| browser_error(&error))?;
            Ok((name, decode_value(&member, ancestors)?))
        })
        .collect::<io::Result<serde_json::Map<_, _>>>()
        .map(Value::Object)
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
            IndexedDBStorageBackend::set(self, store_name, key, encode_json(&value, browser_error)?)
                .await
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
            IndexedDBStorageTransaction::set(
                self,
                store_name,
                key,
                encode_json(&value, browser_error)?,
            )
            .await
        })
    }
    fn delete<'a>(&'a self, store_name: &'a str, key: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(IndexedDBStorageTransaction::delete(self, store_name, key))
    }
}
