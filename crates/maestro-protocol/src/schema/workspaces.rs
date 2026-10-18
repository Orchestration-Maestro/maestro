//! Workspaces protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::common::AgentStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace create params record.
#[schemars(description = "")]
pub struct WorkspaceCreateParams {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace rename params record.
#[schemars(description = "")]
pub struct WorkspaceRenameParams {
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Label.
    #[schemars(description = "")]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace move params record.
#[schemars(description = "")]
pub struct WorkspaceMoveParams {
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Insert index.
    #[schemars(description = "")]
    pub insert_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace move block params record.
#[schemars(description = "")]
pub struct WorkspaceMoveBlockParams {
    /// Workspace ids.
    #[schemars(description = "")]
    pub workspace_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Before workspace id.
    #[schemars(description = "")]
    pub before_workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace report metadata params record.
#[schemars(description = "")]
pub struct WorkspaceReportMetadataParams {
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    #[schemars(schema_with = "super::common::metadata_token_patch_schema")]
    /// Tokens.
    #[schemars(description = "")]
    pub tokens: HashMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seq.
    #[schemars(description = "")]
    pub seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 86_400_000))]
    /// Ttl ms.
    #[schemars(description = "")]
    pub ttl_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace info record.
#[schemars(description = "")]
pub struct WorkspaceInfo {
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
    /// Tab count.
    #[schemars(description = "")]
    pub tab_count: usize,
    /// Active tab id.
    #[schemars(description = "")]
    pub active_tab_id: String,
    /// Agent status.
    #[schemars(description = "")]
    pub agent_status: AgentStatus,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[schemars(schema_with = "super::common::metadata_token_values_schema")]
    /// Tokens.
    #[schemars(description = "")]
    pub tokens: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Worktree.
    #[schemars(description = "")]
    pub worktree: Option<WorkspaceWorktreeInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Workspace worktree info record.
#[schemars(description = "")]
pub struct WorkspaceWorktreeInfo {
    /// Repo key.
    #[schemars(description = "")]
    pub repo_key: String,
    /// Repo name.
    #[schemars(description = "")]
    pub repo_name: String,
    /// Repo root.
    #[schemars(description = "")]
    pub repo_root: String,
    /// Checkout path.
    #[schemars(description = "")]
    pub checkout_path: String,
    /// Is linked worktree.
    #[schemars(description = "")]
    pub is_linked_worktree: bool,
}
