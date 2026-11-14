//! Supplied edit tool data.
use super::OperationFuture;
use crate::Presence;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// Replaceable effects consumed by the native edit owner.
#[derive(Clone)]
pub struct EditOperations {
    /// Reads original file bytes before matching.
    pub read_file: Rc<dyn Fn(String) -> OperationFuture<'static, Vec<u8>>>,
    /// Writes the native edit's UTF-8 result.
    pub write_file: Rc<dyn Fn(String, String) -> OperationFuture<'static, ()>>,
    /// Checks read and write access, preserving structured failure codes.
    pub access: Rc<dyn Fn(String) -> OperationFuture<'static, ()>>,
}
/// Edit construction choices; omitted operations use the local filesystem.
#[derive(Default)]
pub struct EditToolOptions {
    /// Independently retained replacement file effects.
    pub operations: Option<EditOperations>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the edit tool.
pub struct EditToolInput {
    /// Path.
    pub path: String,
    /// Edits.
    pub edits: Vec<ReplaceEdit>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// One authored text replacement.
pub struct ReplaceEdit {
    /// Old text.
    pub old_text: String,
    /// New text.
    pub new_text: String,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Observations supplied by the edit tool.
pub struct EditToolDetails {
    /// Diff.
    pub diff: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// First changed line.
    pub first_changed_line: Presence<f64>,
}
