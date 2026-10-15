//! Replaceable browser storage contracts and database configuration.
use futures_util::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io, rc::Rc};
/// Object-store schema supplied to an adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreConfig {
    /// Collection name.
    pub name: String,
    /// Inline key field, if supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    /// Whether the adapter generates keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_increment: Option<bool>,
    /// Requested indices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<IndexConfig>>,
}
/// An index within an object store.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexConfig {
    /// Index name.
    pub name: String,
    /// Indexed field.
    pub key_path: String,
    /// Optional uniqueness constraint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique: Option<bool>,
}
/// Database schema supplied to an adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedDBConfig {
    /// Database name.
    pub db_name: String,
    /// Database version.
    pub version: f64,
    /// Object-store schemas.
    pub stores: Vec<StoreConfig>,
}

/// Index traversal direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IndexDirection {
    /// Ascending index order.
    #[default]
    Asc,
    /// Descending index order.
    Desc,
}
/// Access requested for a transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionMode {
    /// Read access.
    Readonly,
    /// Read and write access.
    Readwrite,
}
/// Storage usage reported by an adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct QuotaInfo {
    /// Used bytes.
    pub usage: f64,
    /// Available quota in bytes.
    pub quota: f64,
    /// Percentage used.
    pub percent: f64,
}
/// Owned transaction callback with an adapter-defined settlement boundary.
pub type StorageOperation<'a> =
    Box<dyn FnOnce(Rc<dyn StorageTransaction>) -> LocalBoxFuture<'a, io::Result<Value>> + 'a>;
/// Operations scoped to an adapter's transaction.
pub trait StorageTransaction {
    /// Read a collection key, absent or null as no value.
    fn get<'a>(
        &'a self,
        store_name: &'a str,
        key: &'a str,
    ) -> LocalBoxFuture<'a, io::Result<Option<Value>>>;
    /// Write a collection key.
    fn set<'a>(
        &'a self,
        store_name: &'a str,
        key: &'a str,
        value: Value,
    ) -> LocalBoxFuture<'a, io::Result<()>>;
    /// Delete a collection key.
    fn delete<'a>(
        &'a self,
        store_name: &'a str,
        key: &'a str,
    ) -> LocalBoxFuture<'a, io::Result<()>>;
}
/// Multi-collection storage supplied by the caller.
pub trait StorageBackend {
    /// Read a collection key, absent or null as no value.
    fn get<'a>(
        &'a self,
        store_name: &'a str,
        key: &'a str,
    ) -> LocalBoxFuture<'a, io::Result<Option<Value>>>;
    /// Write a collection key.
    fn set<'a>(
        &'a self,
        store_name: &'a str,
        key: &'a str,
        value: Value,
    ) -> LocalBoxFuture<'a, io::Result<()>>;
    /// Delete a collection key.
    fn delete<'a>(
        &'a self,
        store_name: &'a str,
        key: &'a str,
    ) -> LocalBoxFuture<'a, io::Result<()>>;
    /// Enumerate keys with an optional prefix.
    fn keys<'a>(
        &'a self,
        store_name: &'a str,
        prefix: Option<&'a str>,
    ) -> LocalBoxFuture<'a, io::Result<Vec<String>>>;
    /// Enumerate values through an index.
    fn get_all_from_index<'a>(
        &'a self,
        store_name: &'a str,
        index_name: &'a str,
        direction: IndexDirection,
    ) -> LocalBoxFuture<'a, io::Result<Vec<Value>>>;
    /// Remove all keys in a collection.
    fn clear<'a>(&'a self, store_name: &'a str) -> LocalBoxFuture<'a, io::Result<()>>;
    /// Test key presence independently of its value.
    fn has<'a>(&'a self, store_name: &'a str, key: &'a str)
    -> LocalBoxFuture<'a, io::Result<bool>>;
    /// Run a callback within an adapter-owned transaction.
    fn transaction<'a>(
        &'a self,
        store_names: &'a [String],
        mode: TransactionMode,
        operation: StorageOperation<'a>,
    ) -> LocalBoxFuture<'a, io::Result<Value>>;
    /// Report quota information.
    fn get_quota_info(&self) -> LocalBoxFuture<'_, io::Result<QuotaInfo>>;
    /// Request protection from eviction.
    fn request_persistence(&self) -> LocalBoxFuture<'_, io::Result<bool>>;
}
