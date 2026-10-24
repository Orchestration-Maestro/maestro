//! Session protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};

use super::agents::AgentInfo;
use super::panes::{PaneInfo, PaneLayoutSnapshot};
use super::tabs::TabInfo;
use super::workspaces::WorkspaceInfo;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Session snapshot record.
#[schemars(description = "")]
pub struct SessionSnapshot {
    /// Version.
    #[schemars(description = "")]
    pub version: String,
    /// Protocol.
    #[schemars(description = "")]
    pub protocol: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused workspace id.
    #[schemars(description = "")]
    pub focused_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused tab id.
    #[schemars(description = "")]
    pub focused_tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Focused pane id.
    #[schemars(description = "")]
    pub focused_pane_id: Option<String>,
    /// Workspaces.
    #[schemars(description = "")]
    pub workspaces: Vec<WorkspaceInfo>,
    /// Tabs.
    #[schemars(description = "")]
    pub tabs: Vec<TabInfo>,
    /// Panes.
    #[schemars(description = "")]
    pub panes: Vec<PaneInfo>,
    /// Layouts.
    #[schemars(description = "")]
    pub layouts: Vec<PaneLayoutSnapshot>,
    /// Agents.
    #[schemars(description = "")]
    pub agents: Vec<AgentInfo>,
}
