//! Supplied grep tool data.
use super::OperationFuture;
use super::truncate::TruncationResult;
use crate::Presence;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// Replaceable root inspection and context reads; searching remains native.
#[derive(Clone)]
pub struct GrepOperations {
    /// Observes whether the resolved root is a directory.
    pub is_directory: Rc<dyn Fn(String) -> OperationFuture<'static, bool>>,
    /// Returns original context text for a matched absolute file.
    pub read_file: Rc<dyn Fn(String) -> OperationFuture<'static, String>>,
}
/// Grep construction choices; absent observations use native filesystem access.
#[derive(Default)]
pub struct GrepToolOptions {
    /// Independently retained replacement filesystem observations.
    pub operations: Option<GrepOperations>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the grep tool.
pub struct GrepToolInput {
    /// Pattern.
    pub pattern: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Path.
    pub path: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Glob.
    pub glob: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Ignore case.
    pub ignore_case: Presence<bool>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Literal.
    pub literal: Presence<bool>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Context.
    pub context: Presence<f64>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Limit.
    pub limit: Presence<f64>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Observations supplied by the grep tool.
pub struct GrepToolDetails {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Truncation.
    pub truncation: Presence<TruncationResult>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Match limit reached.
    pub match_limit_reached: Presence<f64>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Lines truncated.
    pub lines_truncated: Presence<bool>,
}
