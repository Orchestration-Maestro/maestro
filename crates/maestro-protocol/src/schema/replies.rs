//! Response protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};

use super::actions::{
    ActionCommandLogInfo, ActionInfo, ActionInvocationContext, ActionManifestInfo, ActionPaneInfo,
};
use super::agents::AgentInfo;
use super::common::{ClientWindowTitleReason, NotificationShowReason};
use super::endpoint::ServerCapabilities;
use super::integrations::{
    IntegrationInstallResult, IntegrationTarget, IntegrationUninstallResult,
};
use super::notifications::EventData;
use super::panes::{
    LayoutDescription, PaneEdgesResult, PaneFocusDirectionResult, PaneInfo, PaneLayoutSnapshot,
    PaneMoveResult, PaneNeighborResult, PaneProcessInfo, PaneReadResult, PaneResizeResult,
    PaneSwapResult, PaneZoomResult,
};
use super::snapshots::SessionSnapshot;
use super::tabs::TabInfo;
use super::workspaces::WorkspaceInfo;
use super::worktrees::{WorktreeInfo, WorktreeSourceInfo};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Error body record.
#[schemars(description = "")]
pub struct ErrorBody {
    /// Code.
    #[schemars(description = "")]
    pub code: String,
    /// Message.
    #[schemars(description = "")]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Response result record.
#[schemars(description = "")]
pub enum ResponseResult {
    /// Pong.
    #[schemars(description = "")]
    Pong {
        /// Version.
        #[schemars(description = "")]
        version: String,
        /// Protocol.
        #[schemars(description = "")]
        protocol: u32,
        #[serde(default)]
        /// Capabilities.
        #[schemars(description = "")]
        capabilities: Option<ServerCapabilities>,
    },
    /// Session snapshot.
    #[schemars(description = "")]
    SessionSnapshot {
        /// Snapshot.
        #[schemars(description = "")]
        snapshot: Box<SessionSnapshot>,
    },
    /// Workspace info.
    #[schemars(description = "")]
    WorkspaceInfo {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
    },
    /// Workspace created.
    #[schemars(description = "")]
    WorkspaceCreated {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
        /// Tab.
        #[schemars(description = "")]
        tab: TabInfo,
        /// Root pane.
        #[schemars(description = "")]
        root_pane: PaneInfo,
    },
    /// Workspace list.
    #[schemars(description = "")]
    WorkspaceList {
        /// Workspaces.
        #[schemars(description = "")]
        workspaces: Vec<WorkspaceInfo>,
    },
    /// Worktree list.
    #[schemars(description = "")]
    WorktreeList {
        /// Source.
        #[schemars(description = "")]
        source: WorktreeSourceInfo,
        /// Worktrees.
        #[schemars(description = "")]
        worktrees: Vec<WorktreeInfo>,
    },
    /// Worktree created.
    #[schemars(description = "")]
    WorktreeCreated {
        /// Workspace.
        #[schemars(description = "")]
        workspace: WorkspaceInfo,
        /// Tab.
        #[schemars(description = "")]
        tab: TabInfo,
        /// Root pane.
        #[schemars(description = "")]
        root_pane: PaneInfo,
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
        /// Tab.
        #[schemars(description = "")]
        tab: TabInfo,
        /// Root pane.
        #[schemars(description = "")]
        root_pane: PaneInfo,
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
        /// Path.
        #[schemars(description = "")]
        path: String,
        /// Forced.
        #[schemars(description = "")]
        forced: bool,
    },
    /// Tab info.
    #[schemars(description = "")]
    TabInfo {
        /// Tab.
        #[schemars(description = "")]
        tab: TabInfo,
    },
    /// Tab created.
    #[schemars(description = "")]
    TabCreated {
        /// Tab.
        #[schemars(description = "")]
        tab: TabInfo,
        /// Root pane.
        #[schemars(description = "")]
        root_pane: PaneInfo,
    },
    /// Tab list.
    #[schemars(description = "")]
    TabList {
        /// Tabs.
        #[schemars(description = "")]
        tabs: Vec<TabInfo>,
    },
    /// Agent info.
    #[schemars(description = "")]
    AgentInfo {
        /// Agent.
        #[schemars(description = "")]
        agent: AgentInfo,
    },
    /// Agent started.
    #[schemars(description = "")]
    AgentStarted {
        /// Agent.
        #[schemars(description = "")]
        agent: AgentInfo,
        /// Argv.
        #[schemars(description = "")]
        argv: Vec<String>,
    },
    /// Agent prompted.
    #[schemars(description = "")]
    AgentPrompted {
        /// Agent.
        #[schemars(description = "")]
        agent: AgentInfo,
    },
    /// Agent list.
    #[schemars(description = "")]
    AgentList {
        /// Agents.
        #[schemars(description = "")]
        agents: Vec<AgentInfo>,
    },
    /// Agent view.
    #[schemars(description = "")]
    AgentView {
        /// Active.
        #[schemars(description = "")]
        active: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Source.
        #[schemars(description = "")]
        source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Label.
        #[schemars(description = "")]
        label: Option<String>,
    },
    /// Pane info.
    #[schemars(description = "")]
    PaneInfo {
        /// Pane.
        #[schemars(description = "")]
        pane: PaneInfo,
    },
    /// Pane list.
    #[schemars(description = "")]
    PaneList {
        /// Panes.
        #[schemars(description = "")]
        panes: Vec<PaneInfo>,
    },
    /// Pane current.
    #[schemars(description = "")]
    PaneCurrent {
        /// Pane.
        #[schemars(description = "")]
        pane: PaneInfo,
    },
    /// Pane swap.
    #[schemars(description = "")]
    PaneSwap {
        /// Swap.
        #[schemars(description = "")]
        swap: PaneSwapResult,
    },
    /// Pane move.
    #[schemars(description = "")]
    PaneMove {
        /// Move result.
        #[schemars(description = "")]
        move_result: PaneMoveResult,
    },
    /// Pane zoom.
    #[schemars(description = "")]
    PaneZoom {
        /// Zoom.
        #[schemars(description = "")]
        zoom: PaneZoomResult,
    },
    /// Pane layout.
    #[schemars(description = "")]
    PaneLayout {
        /// Layout.
        #[schemars(description = "")]
        layout: PaneLayoutSnapshot,
    },
    /// Pane process info.
    #[schemars(description = "")]
    PaneProcessInfo {
        /// Process info.
        #[schemars(description = "")]
        process_info: PaneProcessInfo,
    },
    /// Layout export.
    #[schemars(description = "")]
    LayoutExport {
        /// Layout.
        #[schemars(description = "")]
        layout: LayoutDescription,
    },
    /// Layout apply.
    #[schemars(description = "")]
    LayoutApply {
        /// Layout.
        #[schemars(description = "")]
        layout: LayoutDescription,
    },
    /// Layout split ratio set.
    #[schemars(description = "")]
    LayoutSplitRatioSet {
        /// Layout.
        #[schemars(description = "")]
        layout: LayoutDescription,
    },
    /// Pane neighbor.
    #[schemars(description = "")]
    PaneNeighbor {
        /// Neighbor.
        #[schemars(description = "")]
        neighbor: PaneNeighborResult,
    },
    /// Pane edges.
    #[schemars(description = "")]
    PaneEdges {
        /// Edges.
        #[schemars(description = "")]
        edges: PaneEdgesResult,
    },
    /// Pane focus direction.
    #[schemars(description = "")]
    PaneFocusDirection {
        /// Focus.
        #[schemars(description = "")]
        focus: PaneFocusDirectionResult,
    },
    /// Pane resize.
    #[schemars(description = "")]
    PaneResize {
        /// Resize.
        #[schemars(description = "")]
        resize: PaneResizeResult,
    },
    /// Pane read.
    #[schemars(description = "")]
    PaneRead {
        /// Read.
        #[schemars(description = "")]
        read: PaneReadResult,
    },
    /// Pane graphics frame ack.
    #[schemars(description = "")]
    PaneGraphicsFrameAck {
        /// Sequence.
        #[schemars(description = "")]
        sequence: u64,
        /// Revision.
        #[schemars(description = "")]
        revision: u64,
    },
    /// Pane graphics info.
    #[schemars(description = "")]
    PaneGraphicsInfo {
        /// Cell width px.
        #[schemars(description = "")]
        cell_width_px: u32,
        /// Cell height px.
        #[schemars(description = "")]
        cell_height_px: u32,
        /// True only when this pane is on the currently rendered terminal surface.
        pane_visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// File frame directory.
        #[schemars(description = "")]
        file_frame_directory: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        /// File frame formats.
        #[schemars(description = "")]
        file_frame_formats: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// File frame max bytes.
        #[schemars(description = "")]
        file_frame_max_bytes: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// File frame direct max bytes.
        #[schemars(description = "")]
        file_frame_direct_max_bytes: Option<usize>,
        /// Accepts damage metadata while still consuming a complete canonical file.
        #[serde(default)]
        file_frame_damage: bool,
        #[serde(default)]
        /// Max layers per pane.
        #[schemars(description = "")]
        max_layers_per_pane: usize,
        #[serde(default)]
        /// Pixel mouse.
        #[schemars(description = "")]
        pixel_mouse: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// File frame transport.
        #[schemars(description = "")]
        file_frame_transport: Option<String>,
    },
    /// Agent explain.
    #[schemars(description = "")]
    AgentExplain {
        /// Explain.
        #[schemars(description = "")]
        explain: serde_json::Value,
    },
    /// Subscription started.
    #[schemars(description = "")]
    SubscriptionStarted {},
    /// Wait matched.
    #[schemars(description = "")]
    WaitMatched {
        /// Event.
        #[schemars(description = "")]
        event: EventData,
    },
    /// Output matched.
    #[schemars(description = "")]
    OutputMatched {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
        /// Revision.
        #[schemars(description = "")]
        revision: u64,
        /// Matched line.
        #[schemars(description = "")]
        matched_line: Option<String>,
        /// Read.
        #[schemars(description = "")]
        read: PaneReadResult,
    },
    /// Notification show.
    #[schemars(description = "")]
    NotificationShow {
        /// Shown.
        #[schemars(description = "")]
        shown: bool,
        /// Reason.
        #[schemars(description = "")]
        reason: NotificationShowReason,
    },
    /// Client window title.
    #[schemars(description = "")]
    ClientWindowTitle {
        /// Changed.
        #[schemars(description = "")]
        changed: bool,
        /// Reason.
        #[schemars(description = "")]
        reason: ClientWindowTitleReason,
    },
    /// Integration install.
    #[schemars(description = "")]
    IntegrationInstall {
        /// Target.
        #[schemars(description = "")]
        target: IntegrationTarget,
        /// Details.
        #[schemars(description = "")]
        details: IntegrationInstallResult,
    },
    /// Integration uninstall.
    #[schemars(description = "")]
    IntegrationUninstall {
        /// Target.
        #[schemars(description = "")]
        target: IntegrationTarget,
        /// Details.
        #[schemars(description = "")]
        details: IntegrationUninstallResult,
    },
    /// Agent manifest reload.
    #[schemars(description = "")]
    AgentManifestReload {
        /// Manifests.
        #[schemars(description = "")]
        manifests: Vec<AgentManifestInfo>,
    },
    /// Agent manifest status.
    #[schemars(description = "")]
    AgentManifestStatus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Last check unix.
        #[schemars(description = "")]
        last_check_unix: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// Last result.
        #[schemars(description = "")]
        last_result: Option<String>,
        /// Manifests.
        #[schemars(description = "")]
        manifests: Vec<AgentManifestInfo>,
    },
    /// Action linked.
    #[schemars(description = "")]
    ActionManifestLinked {
        /// Action.
        #[schemars(description = "")]
        manifest: ActionManifestInfo,
    },
    /// Action list.
    #[schemars(description = "")]
    ActionManifestList {
        /// Action manifests.
        #[schemars(description = "")]
        manifests: Vec<ActionManifestInfo>,
    },
    /// Action unlinked.
    #[schemars(description = "")]
    ActionManifestUnlinked {
        /// Manifest identifier.
        #[schemars(description = "")]
        manifest_id: String,
        /// Removed.
        #[schemars(description = "")]
        removed: bool,
    },
    /// Action enabled.
    #[schemars(description = "")]
    ActionManifestEnabled {
        /// Action.
        #[schemars(description = "")]
        manifest: ActionManifestInfo,
    },
    /// Action disabled.
    #[schemars(description = "")]
    ActionManifestDisabled {
        /// Action.
        #[schemars(description = "")]
        manifest: ActionManifestInfo,
    },
    /// Action list.
    #[schemars(description = "")]
    ActionList {
        /// Actions.
        #[schemars(description = "")]
        actions: Vec<ActionInfo>,
    },
    /// Action invoked.
    #[schemars(description = "")]
    ActionInvoked {
        /// Action.
        #[schemars(description = "")]
        action: ActionInfo,
        /// Context.
        #[schemars(description = "")]
        context: ActionInvocationContext,
        /// Log.
        #[schemars(description = "")]
        log: ActionCommandLogInfo,
    },
    /// Action log list.
    #[schemars(description = "")]
    ActionLogList {
        /// Logs.
        #[schemars(description = "")]
        logs: Vec<ActionCommandLogInfo>,
    },
    /// Action pane opened.
    #[schemars(description = "")]
    ActionPaneOpened {
        /// Action pane.
        #[schemars(description = "")]
        action_pane: ActionPaneInfo,
    },
    /// Action pane focused.
    #[schemars(description = "")]
    ActionPaneFocused {
        /// Action pane.
        #[schemars(description = "")]
        action_pane: ActionPaneInfo,
    },
    /// Action pane closed.
    #[schemars(description = "")]
    ActionPaneClosed {
        /// Pane id.
        #[schemars(description = "")]
        pane_id: String,
    },
    /// Config reload.
    #[schemars(description = "")]
    ConfigReload {
        /// Status.
        #[schemars(description = "")]
        status: crate::shared::ConfigReloadStatus,
        /// Diagnostics.
        #[schemars(description = "")]
        diagnostics: Vec<String>,
    },
    /// Ok.
    #[schemars(description = "")]
    Ok {},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Agent manifest info record.
#[schemars(description = "")]
pub struct AgentManifestInfo {
    /// Agent.
    #[schemars(description = "")]
    pub agent: String,
    /// Source.
    #[schemars(description = "")]
    pub source: String,
    /// Source kind.
    #[schemars(description = "")]
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Active version.
    #[schemars(description = "")]
    pub active_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Cached remote version.
    #[schemars(description = "")]
    pub cached_remote_version: Option<String>,
    /// Local override shadowing remote.
    #[schemars(description = "")]
    pub local_override_shadowing_remote: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Remote update result.
    #[schemars(description = "")]
    pub remote_update_result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Remote update error.
    #[schemars(description = "")]
    pub remote_update_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Remote last checked unix.
    #[schemars(description = "")]
    pub remote_last_checked_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Warning.
    #[schemars(description = "")]
    pub warning: Option<String>,
}
