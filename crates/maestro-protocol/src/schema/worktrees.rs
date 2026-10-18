//! Worktrees protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Worktree list params record.
#[schemars(description = "")]
pub struct WorktreeListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Worktree create params record.
#[schemars(description = "")]
pub struct WorktreeCreateParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Branch.
    #[schemars(description = "")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Base.
    #[schemars(description = "")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Path.
    #[schemars(description = "")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Label.
    #[schemars(description = "")]
    pub label: Option<String>,
    #[serde(default)]
    /// Focus.
    #[schemars(description = "")]
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Worktree open params record.
#[schemars(description = "")]
pub struct WorktreeOpenParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cwd.
    #[schemars(description = "")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Path.
    #[schemars(description = "")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Branch.
    #[schemars(description = "")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Label.
    #[schemars(description = "")]
    pub label: Option<String>,
    #[serde(default)]
    /// Focus.
    #[schemars(description = "")]
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Worktree remove params record.
#[schemars(description = "")]
pub struct WorktreeRemoveParams {
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    #[serde(default)]
    /// Force.
    #[schemars(description = "")]
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Worktree source info record.
#[schemars(description = "")]
pub struct WorktreeSourceInfo {
    /// Repo key.
    #[schemars(description = "")]
    pub repo_key: String,
    /// Repo name.
    #[schemars(description = "")]
    pub repo_name: String,
    /// Repo root.
    #[schemars(description = "")]
    pub repo_root: String,
    /// Source checkout path.
    #[schemars(description = "")]
    pub source_checkout_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Source workspace id.
    #[schemars(description = "")]
    pub source_workspace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Worktree info record.
#[schemars(description = "")]
pub struct WorktreeInfo {
    /// Path.
    #[schemars(description = "")]
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Branch.
    #[schemars(description = "")]
    pub branch: Option<String>,
    /// Is bare.
    #[schemars(description = "")]
    pub is_bare: bool,
    /// Is detached.
    #[schemars(description = "")]
    pub is_detached: bool,
    /// Is prunable.
    #[schemars(description = "")]
    pub is_prunable: bool,
    /// Is linked worktree.
    #[schemars(description = "")]
    pub is_linked_worktree: crate::shared::Flag,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Open workspace id.
    #[schemars(description = "")]
    pub open_workspace_id: Option<String>,
    /// Label.
    #[schemars(description = "")]
    pub label: String,
}
