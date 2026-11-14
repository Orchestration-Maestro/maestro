//! Supplied find tool data.
use super::OperationFuture;
use super::limits::TruncationResult;
use crate::Presence;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub use maestro_interchange::tool::types::FindGlobOptions;
/// Replaceable existence and glob effects; path and output policy remain native.
#[derive(Clone)]
pub struct FindOperations {
    /// Observes the resolved search root.
    pub exists: Rc<dyn Fn(String) -> OperationFuture<'static, bool>>,
    /// Returns original relative or absolute paths in producer order.
    pub glob: Rc<dyn Fn(String, String, FindGlobOptions) -> OperationFuture<'static, Vec<String>>>,
}
/// Find construction choices; absent operations use the application's native search owner.
#[derive(Default)]
pub struct FindToolOptions {
    /// Independently retained replacement search observations.
    pub operations: Option<FindOperations>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the find tool.
pub struct FindToolInput {
    /// Pattern.
    pub pattern: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Path.
    pub path: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Limit.
    pub limit: Presence<f64>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Observations supplied by the find tool.
pub struct FindToolDetails {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Truncation.
    pub truncation: Presence<TruncationResult>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Result limit reached.
    pub result_limit_reached: Presence<f64>,
}
