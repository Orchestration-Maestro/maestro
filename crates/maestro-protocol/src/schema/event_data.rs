//! Events protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::common::AgentStatus;
use super::events::EventKind;
use super::panes::{PaneInfo, PaneReadResult, PaneScrollInfo};
use super::tabs::TabInfo;
use super::workspaces::WorkspaceInfo;
use super::worktrees::WorktreeInfo;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Event envelope record.
#[schemars(description = "")]
pub struct EventEnvelope {
    /// Event.
    #[schemars(description = "")]
    pub event: EventKind,
    /// Data.
    #[schemars(description = "")]
    pub data: EventData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Subscription event kind record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum SubscriptionEventKind {
    #[serde(rename = "pane.output_matched")]
    /// Pane output matched.
    #[schemars(description = "")]
    PaneOutputMatched,
    #[serde(rename = "pane.agent_status_changed")]
    /// Pane agent status changed.
    #[schemars(description = "")]
    PaneAgentStatusChanged,
    #[serde(rename = "pane.scroll_changed")]
    /// Scroll changed.
    #[schemars(description = "")]
    ScrollChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Subscription event envelope record.
#[schemars(description = "")]
pub struct SubscriptionEventEnvelope {
    /// Event.
    #[schemars(description = "")]
    pub event: SubscriptionEventKind,
    /// Data.
    #[schemars(description = "")]
    pub data: SubscriptionEventData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
/// Subscription event data record.
#[schemars(description = "")]
pub enum SubscriptionEventData {
    /// Pane output matched.
    #[schemars(description = "")]
    PaneOutputMatched(PaneOutputMatchedEvent),
    /// Pane agent status changed.
    #[schemars(description = "")]
    PaneAgentStatusChanged(PaneAgentStatusChangedEvent),
    /// Scroll changed.
    #[schemars(description = "")]
    ScrollChanged(PaneScrollChangedEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane output matched event record.
#[schemars(description = "")]
pub struct PaneOutputMatchedEvent {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Matched line.
    #[schemars(description = "")]
    pub matched_line: String,
    /// Read.
    #[schemars(description = "")]
    pub read: PaneReadResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane agent status changed event record.
#[schemars(description = "")]
pub struct PaneAgentStatusChangedEvent {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Agent status.
    #[schemars(description = "")]
    pub agent_status: AgentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Agent.
    #[schemars(description = "")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Title.
    #[schemars(description = "")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Display agent.
    #[schemars(description = "")]
    pub display_agent: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    /// State labels.
    #[schemars(description = "")]
    pub state_labels: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane scroll changed event record.
#[schemars(description = "")]
pub struct PaneScrollChangedEvent {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Workspace id.
    #[schemars(description = "")]
    pub workspace_id: String,
    /// Scroll.
    #[schemars(description = "")]
    pub scroll: PaneScrollInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Event data record.
#[schemars(description = "")]
pub enum EventData {
    /// Workspace created.
    #[schemars(description = "")]
    WorkspaceCreated {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
    },
    /// Workspace updated.
    #[schemars(description = "")]
    WorkspaceUpdated {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
    },
    /// Workspace metadata updated.
    #[schemars(description = "")]
    WorkspaceMetadataUpdated {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
    },
    /// Workspace closed.
    #[schemars(description = "")]
    WorkspaceClosed {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Workspace.
        #[schemars(description = "")]
        workspace: Option<WorkspaceInfo>,
    },
    /// Workspace renamed.
    #[schemars(description = "")]
    WorkspaceRenamed {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        /// Label.
        #[schemars(description = "")]
        label: String,
    },
    /// Workspace moved.
    #[schemars(description = "")]
    WorkspaceMoved {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        /// Insert index.
        #[schemars(description = "")]
        insert_index: usize,
        /// Workspaces.
        #[schemars(description = "")]
        workspaces: Vec<WorkspaceInfo>,
    },
    /// Workspace reordered.
    #[schemars(description = "")]
    WorkspaceReordered {
        /// Workspace ids.
        #[schemars(description = "")]
        workspace_ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Before workspace id.
        #[schemars(description = "")]
        before_workspace_id: Option<String>,
        /// Workspaces.
        #[schemars(description = "")]
        workspaces: Vec<WorkspaceInfo>,
    },
    /// Workspace focused.
    #[schemars(description = "")]
    WorkspaceFocused {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Worktree created.
    #[schemars(description = "")]
    WorktreeCreated {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
        /// Worktree.
        #[schemars(description = "")]
        worktree: WorktreeInfo,
    },
    /// Worktree opened.
    #[schemars(description = "")]
    WorktreeOpened {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
        /// Worktree.
        #[schemars(description = "")]
        worktree: WorktreeInfo,
        /// Already open.
        #[schemars(description = "")]
        already_open: bool,
    },
    /// Worktree removed.
    #[schemars(description = "")]
    WorktreeRemoved {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Workspace.
        #[schemars(description = "")]
        workspace: Option<WorkspaceInfo>,
        /// Worktree.
        #[schemars(description = "")]
        worktree: WorktreeInfo,
        /// Forced.
        #[schemars(description = "")]
        forced: bool,
    },
    /// Tab created.
    #[schemars(description = "")]
    TabCreated {
        /// Tab.
        #[schemars(description = "")]
        tab: TabInfo,
    },
    /// Tab closed.
    #[schemars(description = "")]
    TabClosed {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Tab renamed.
    #[schemars(description = "")]
    TabRenamed {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        /// Label.
        #[schemars(description = "")]
        label: String,
    },
    /// Tab moved.
    #[schemars(description = "")]
    TabMoved {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        /// Insert index.
        #[schemars(description = "")]
        insert_index: usize,
        /// Tabs.
        #[schemars(description = "")]
        tabs: Vec<TabInfo>,
    },
    /// Tab focused.
    #[schemars(description = "")]
    TabFocused {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Pane created.
    #[schemars(description = "")]
    PaneCreated {
        /// Pane.
        #[schemars(description = "")]
        pane: PaneInfo,
    },
    /// Pane closed.
    #[schemars(description = "")]
    PaneClosed {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Pane updated.
    #[schemars(description = "")]
    PaneUpdated {
        /// Pane.
        #[schemars(description = "")]
        pane: PaneInfo,
    },
    /// Pane focused.
    #[schemars(description = "")]
    PaneFocused {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Pane moved.
    #[schemars(description = "")]
    PaneMoved {
        /// Previous pane id.
        #[schemars(description = "")]
        previous_pane_id: String,
        /// Previous workspace id.
        #[schemars(description = "")]
        previous_workspace_id: String,
        /// Previous tab id.
        #[schemars(description = "")]
        previous_tab_id: String,
        /// Pane.
        #[schemars(description = "")]
        pane: Box<PaneInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Created workspace.
        #[schemars(description = "")]
        created_workspace: Option<WorkspaceInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Created tab.
        #[schemars(description = "")]
        created_tab: Option<TabInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Closed workspace id.
        #[schemars(description = "")]
        closed_workspace_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Closed tab id.
        #[schemars(description = "")]
        closed_tab_id: Option<String>,
    },
    /// Pane output changed.
    #[schemars(description = "")]
    PaneOutputChanged {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        /// Revision.
        #[schemars(description = "")]
        revision: u64,
    },
    /// Pane exited.
    #[schemars(description = "")]
    PaneExited {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Pane agent detected.
    #[schemars(description = "")]
    PaneAgentDetected {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Agent.
        #[schemars(description = "")]
        agent: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        /// Released.
        #[schemars(description = "")]
        released: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Final status.
        #[schemars(description = "")]
        final_status: Option<AgentStatus>,
    },
    /// Pane agent status changed.
    #[schemars(description = "")]
    PaneAgentStatusChanged {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        /// Agent status.
        #[schemars(description = "")]
        agent_status: AgentStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Agent.
        #[schemars(description = "")]
        agent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Title.
        #[schemars(description = "")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Display agent.
        #[schemars(description = "")]
        display_agent: Option<String>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        /// State labels.
        #[schemars(description = "")]
        state_labels: HashMap<String, String>,
    },
    /// Layout updated.
    #[schemars(description = "")]
    LayoutUpdated {
        /// Layout.
        #[schemars(description = "")]
        layout: super::panes::PaneLayoutSnapshot,
    },
}
