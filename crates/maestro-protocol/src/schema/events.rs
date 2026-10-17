//! Events protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.

use serde::{Deserialize, Serialize};

use super::common::{AgentStatus, ReadSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Events subscribe params record.
#[schemars(description = "")]
pub struct EventsSubscribeParams {
    /// Subscriptions.
    #[schemars(description = "")]
    pub subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type")]
/// Subscription record.
#[schemars(description = "")]
pub enum Subscription {
    #[serde(rename = "workspace.created")]
    /// Workspace created.
    #[schemars(description = "")]
    WorkspaceCreated {},
    #[serde(rename = "workspace.updated")]
    /// Workspace updated.
    #[schemars(description = "")]
    WorkspaceUpdated {},
    #[serde(rename = "workspace.metadata_updated")]
    /// Workspace metadata updated.
    #[schemars(description = "")]
    WorkspaceMetadataUpdated {},
    #[serde(rename = "workspace.renamed")]
    /// Workspace renamed.
    #[schemars(description = "")]
    WorkspaceRenamed {},
    #[serde(rename = "workspace.moved")]
    /// Workspace moved.
    #[schemars(description = "")]
    WorkspaceMoved {},
    #[serde(rename = "workspace.reordered")]
    /// Workspace reordered.
    #[schemars(description = "")]
    WorkspaceReordered {},
    #[serde(rename = "workspace.closed")]
    /// Workspace closed.
    #[schemars(description = "")]
    WorkspaceClosed {},
    #[serde(rename = "workspace.focused")]
    /// Workspace focused.
    #[schemars(description = "")]
    WorkspaceFocused {},
    #[serde(rename = "worktree.created")]
    /// Worktree created.
    #[schemars(description = "")]
    WorktreeCreated {},
    #[serde(rename = "worktree.opened")]
    /// Worktree opened.
    #[schemars(description = "")]
    WorktreeOpened {},
    #[serde(rename = "worktree.removed")]
    /// Worktree removed.
    #[schemars(description = "")]
    WorktreeRemoved {},
    #[serde(rename = "tab.created")]
    /// Tab created.
    #[schemars(description = "")]
    TabCreated {},
    #[serde(rename = "tab.closed")]
    /// Tab closed.
    #[schemars(description = "")]
    TabClosed {},
    #[serde(rename = "tab.focused")]
    /// Tab focused.
    #[schemars(description = "")]
    TabFocused {},
    #[serde(rename = "tab.renamed")]
    /// Tab renamed.
    #[schemars(description = "")]
    TabRenamed {},
    #[serde(rename = "tab.moved")]
    /// Tab moved.
    #[schemars(description = "")]
    TabMoved {},
    #[serde(rename = "pane.created")]
    /// Pane created.
    #[schemars(description = "")]
    PaneCreated {},
    #[serde(rename = "pane.closed")]
    /// Pane closed.
    #[schemars(description = "")]
    PaneClosed {},
    #[serde(rename = "pane.updated")]
    /// Pane updated.
    #[schemars(description = "")]
    PaneUpdated {},
    #[serde(rename = "pane.focused")]
    /// Pane focused.
    #[schemars(description = "")]
    PaneFocused {},
    #[serde(rename = "pane.moved")]
    /// Pane moved.
    #[schemars(description = "")]
    PaneMoved {},
    #[serde(rename = "pane.exited")]
    /// Pane exited.
    #[schemars(description = "")]
    PaneExited {},
    #[serde(rename = "pane.agent_detected")]
    /// Pane agent detected.
    #[schemars(description = "")]
    PaneAgentDetected {},
    #[serde(rename = "pane.output_matched")]
    /// Pane output matched.
    #[schemars(description = "")]
    PaneOutputMatched {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Source.
        #[schemars(description = "")]
        source: ReadSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Lines.
        #[schemars(description = "")]
        lines: Option<u32>,
        /// Match.
        #[schemars(description = "")]
        r#match: OutputMatch,
        #[serde(default = "super::common::default_true")]
        /// Strip ansi.
        #[schemars(description = "")]
        strip_ansi: bool,
    },
    #[serde(rename = "pane.agent_status_changed")]
    /// Pane agent status changed.
    #[schemars(description = "")]
    PaneAgentStatusChanged {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Agent status.
        #[schemars(description = "")]
        agent_status: Option<AgentStatus>,
    },
    #[serde(rename = "pane.scroll_changed")]
    /// Pane scroll changed.
    #[schemars(description = "")]
    PaneScrollChanged {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
    },
    #[serde(rename = "layout.updated")]
    /// Layout updated.
    #[schemars(description = "")]
    LayoutUpdated {},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Events wait params record.
#[schemars(description = "")]
pub struct EventsWaitParams {
    /// Match event.
    #[schemars(description = "")]
    pub match_event: EventMatch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Timeout ms.
    #[schemars(description = "")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane wait for output params record.
#[schemars(description = "")]
pub struct PaneWaitForOutputParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    /// Source.
    #[schemars(description = "")]
    pub source: ReadSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Lines.
    #[schemars(description = "")]
    pub lines: Option<u32>,
    /// Match.
    #[schemars(description = "")]
    pub r#match: OutputMatch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Timeout ms.
    #[schemars(description = "")]
    pub timeout_ms: Option<u64>,
    #[serde(default = "super::common::default_true")]
    /// Strip ansi.
    #[schemars(description = "")]
    pub strip_ansi: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Output match record.
#[schemars(description = "")]
pub enum OutputMatch {
    /// Substring.
    #[schemars(description = "")]
    Substring {
        /// Value.
        #[schemars(description = "")]
        value: String,
    },
    /// Regex.
    #[schemars(description = "")]
    Regex {
        /// Value.
        #[schemars(description = "")]
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
/// Event match record.
#[schemars(description = "")]
pub enum EventMatch {
    /// Workspace created.
    #[schemars(description = "")]
    WorkspaceCreated {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: Option<String>,
    },
    /// Workspace updated.
    #[schemars(description = "")]
    WorkspaceUpdated {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Workspace closed.
    #[schemars(description = "")]
    WorkspaceClosed {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Workspace renamed.
    #[schemars(description = "")]
    WorkspaceRenamed {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Label.
        #[schemars(description = "")]
        label: Option<String>,
    },
    /// Workspace moved.
    #[schemars(description = "")]
    WorkspaceMoved {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Workspace focused.
    #[schemars(description = "")]
    WorkspaceFocused {
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: String,
    },
    /// Tab created.
    #[schemars(description = "")]
    TabCreated {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Tab id.
        #[schemars(description = "")]
        tab_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: Option<String>,
    },
    /// Tab closed.
    #[schemars(description = "")]
    TabClosed {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
    },
    /// Tab renamed.
    #[schemars(description = "")]
    TabRenamed {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Label.
        #[schemars(description = "")]
        label: Option<String>,
    },
    /// Tab moved.
    #[schemars(description = "")]
    TabMoved {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
    },
    /// Tab focused.
    #[schemars(description = "")]
    TabFocused {
        /// Tab id.
        #[schemars(description = "")]
        tab_id: String,
    },
    /// Pane created.
    #[schemars(description = "")]
    PaneCreated {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Pane id.
        #[schemars(description = "")]
        pane_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Workspace id.
        #[schemars(description = "")]
        workspace_id: Option<String>,
    },
    /// Pane closed.
    #[schemars(description = "")]
    PaneClosed {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
    },
    /// Pane focused.
    #[schemars(description = "")]
    PaneFocused {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
    },
    /// Pane moved.
    #[schemars(description = "")]
    PaneMoved {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
    },
    /// Pane output changed.
    #[schemars(description = "")]
    PaneOutputChanged {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Min revision.
        #[schemars(description = "")]
        min_revision: Option<u64>,
    },
    /// Pane exited.
    #[schemars(description = "")]
    PaneExited {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
    },
    /// Pane agent detected.
    #[schemars(description = "")]
    PaneAgentDetected {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Agent.
        #[schemars(description = "")]
        agent: Option<String>,
    },
    /// Pane agent status changed.
    #[schemars(description = "")]
    PaneAgentStatusChanged {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Agent status.
        #[schemars(description = "")]
        agent_status: AgentStatus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Event kind record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum EventKind {
    /// Workspace created.
    #[schemars(description = "")]
    WorkspaceCreated,
    /// Workspace updated.
    #[schemars(description = "")]
    WorkspaceUpdated,
    /// Workspace metadata updated.
    #[schemars(description = "")]
    WorkspaceMetadataUpdated,
    /// Workspace closed.
    #[schemars(description = "")]
    WorkspaceClosed,
    /// Workspace renamed.
    #[schemars(description = "")]
    WorkspaceRenamed,
    /// Workspace moved.
    #[schemars(description = "")]
    WorkspaceMoved,
    /// Workspace reordered.
    #[schemars(description = "")]
    WorkspaceReordered,
    /// Workspace focused.
    #[schemars(description = "")]
    WorkspaceFocused,
    /// Worktree created.
    #[schemars(description = "")]
    WorktreeCreated,
    /// Worktree opened.
    #[schemars(description = "")]
    WorktreeOpened,
    /// Worktree removed.
    #[schemars(description = "")]
    WorktreeRemoved,
    /// Tab created.
    #[schemars(description = "")]
    TabCreated,
    /// Tab closed.
    #[schemars(description = "")]
    TabClosed,
    /// Tab renamed.
    #[schemars(description = "")]
    TabRenamed,
    /// Tab moved.
    #[schemars(description = "")]
    TabMoved,
    /// Tab focused.
    #[schemars(description = "")]
    TabFocused,
    /// Pane created.
    #[schemars(description = "")]
    PaneCreated,
    /// Pane closed.
    #[schemars(description = "")]
    PaneClosed,
    /// Pane updated.
    #[schemars(description = "")]
    PaneUpdated,
    /// Pane focused.
    #[schemars(description = "")]
    PaneFocused,
    /// Pane moved.
    #[schemars(description = "")]
    PaneMoved,
    /// Pane output changed.
    #[schemars(description = "")]
    PaneOutputChanged,
    /// Pane exited.
    #[schemars(description = "")]
    PaneExited,
    /// Pane agent detected.
    #[schemars(description = "")]
    PaneAgentDetected,
    /// Pane agent status changed.
    #[schemars(description = "")]
    PaneAgentStatusChanged,
    /// Layout updated.
    #[schemars(description = "")]
    LayoutUpdated,
}

impl EventKind {
    /// Dot name.
    #[must_use]
    pub fn dot_name(self) -> &'static str {
        match self {
            EventKind::WorkspaceCreated => "workspace.created",
            EventKind::WorkspaceUpdated => "workspace.updated",
            EventKind::WorkspaceMetadataUpdated => "workspace.metadata_updated",
            EventKind::WorkspaceClosed => "workspace.closed",
            EventKind::WorkspaceRenamed => "workspace.renamed",
            EventKind::WorkspaceMoved => "workspace.moved",
            EventKind::WorkspaceReordered => "workspace.reordered",
            EventKind::WorkspaceFocused => "workspace.focused",
            EventKind::WorktreeCreated => "worktree.created",
            EventKind::WorktreeOpened => "worktree.opened",
            EventKind::WorktreeRemoved => "worktree.removed",
            EventKind::TabCreated => "tab.created",
            EventKind::TabClosed => "tab.closed",
            EventKind::TabRenamed => "tab.renamed",
            EventKind::TabMoved => "tab.moved",
            EventKind::TabFocused => "tab.focused",
            EventKind::PaneCreated => "pane.created",
            EventKind::PaneClosed => "pane.closed",
            EventKind::PaneUpdated => "pane.updated",
            EventKind::PaneFocused => "pane.focused",
            EventKind::PaneMoved => "pane.moved",
            EventKind::PaneOutputChanged => "pane.output_changed",
            EventKind::PaneExited => "pane.exited",
            EventKind::PaneAgentDetected => "pane.agent_detected",
            EventKind::PaneAgentStatusChanged => "pane.agent_status_changed",
            EventKind::LayoutUpdated => "layout.updated",
        }
    }
}

/// Plugin hook event kinds.
pub const PLUGIN_HOOK_EVENT_KINDS: &[EventKind] = &[
    EventKind::WorkspaceCreated,
    EventKind::WorkspaceUpdated,
    EventKind::WorkspaceClosed,
    EventKind::WorkspaceRenamed,
    EventKind::WorkspaceMoved,
    EventKind::WorkspaceReordered,
    EventKind::WorkspaceFocused,
    EventKind::WorktreeCreated,
    EventKind::WorktreeOpened,
    EventKind::WorktreeRemoved,
    EventKind::TabCreated,
    EventKind::TabClosed,
    EventKind::TabRenamed,
    EventKind::TabMoved,
    EventKind::TabFocused,
    EventKind::PaneCreated,
    EventKind::PaneClosed,
    EventKind::PaneFocused,
    EventKind::PaneMoved,
    EventKind::PaneExited,
    EventKind::PaneAgentDetected,
    EventKind::PaneAgentStatusChanged,
];

/// Event names that manifest `[[events]] on` hooks can reference. This is
/// intentionally narrower than `EventKind` until high-volume output-change hook
/// semantics are implemented.
#[must_use]
pub fn plugin_hook_event_names() -> Vec<&'static str> {
    PLUGIN_HOOK_EVENT_KINDS
        .iter()
        .copied()
        .map(EventKind::dot_name)
        .collect()
}

pub use super::event_data::*;
