//! Tabs protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::common::AgentStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Tab create params record.
#[schemars(description = "")]
pub struct TabCreateParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default)]
    /// Focus.
    #[schemars(description = "")]
    pub focus: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Label.
    #[schemars(description = "")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// Env.
    #[schemars(description = "")]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Tab list params record.
#[schemars(description = "")]
pub struct TabListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Tab rename params record.
#[schemars(description = "")]
pub struct TabRenameParams {
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Label.
    #[schemars(description = "")]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Tab move params record.
#[schemars(description = "")]
pub struct TabMoveParams {
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Insert index.
    #[schemars(description = "")]
    pub insert_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Tab info record.
#[schemars(description = "")]
pub struct TabInfo {
    /// Tab id.
    #[schemars(description = "")]
    pub tab_id: String,
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Number.
    #[schemars(description = "")]
    pub number: usize,
    /// Label.
    #[schemars(description = "")]
    pub label: String,
    /// Focused.
    #[schemars(description = "")]
    pub focused: bool,
    /// Pane count.
    #[schemars(description = "")]
    pub pane_count: usize,
    /// Agent status.
    #[schemars(description = "")]
    pub agent_status: AgentStatus,
}
