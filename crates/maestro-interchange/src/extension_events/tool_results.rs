//! Tool handler answers and user shell-command data.
use crate::presence::Presence;
use crate::types::UserBlock;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `UserBashEvent` data.
pub struct UserBashEvent {
    /// Command.
    pub command: String,
    /// Exclude from context.
    pub exclude_from_context: bool,
    /// Cwd.
    pub cwd: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `UserBashEventResult` data.
#[serde(bound(deserialize = "R: Deserialize<'de>"))]
pub struct UserBashEventResult<O, R> {
    /// Custom execution capability, carried outside the JSON payload.
    #[serde(skip)]
    pub operations: Option<O>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Result.
    pub result: Presence<R>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `ToolCallEventResult` data.
pub struct ToolCallEventResult {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Block.
    pub block: Presence<bool>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Reason.
    pub reason: Presence<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `ToolResultEventResult` data.
#[serde(bound(deserialize = "D: Deserialize<'de>"))]
pub struct ToolResultEventResult<D> {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Content.
    pub content: Presence<Vec<UserBlock>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Details.
    pub details: Presence<D>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Is error.
    pub is_error: Presence<bool>,
}
