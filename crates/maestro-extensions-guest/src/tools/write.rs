//! Supplied write tool data.
use super::OperationFuture;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// Replaceable effects consumed by the native write owner.
#[derive(Clone)]
pub struct WriteOperations {
    /// Replaces the target with the original UTF-8 content.
    pub write_file: Rc<dyn Fn(String, String) -> OperationFuture<'static, ()>>,
    /// Creates the target parent directory recursively.
    pub mkdir: Rc<dyn Fn(String) -> OperationFuture<'static, ()>>,
}
/// Write construction choices; omitted operations use native file access.
#[derive(Default)]
pub struct WriteToolOptions {
    /// Independently retained replacement file effects.
    pub operations: Option<WriteOperations>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the write tool.
pub struct WriteToolInput {
    /// Path.
    pub path: String,
    /// Content.
    pub content: String,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
