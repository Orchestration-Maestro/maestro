//! Shared RPC commands and correlated responses.
use crate::schema::{
    ActionInvokeParams, ActionListParams, ActionLogListParams, ActionManifestLinkParams,
    ActionManifestListParams, ActionManifestSetEnabledParams, ActionManifestUnlinkParams,
    ActionPaneCloseParams, ActionPaneFocusParams, ActionPaneOpenParams, AgentPromptParams,
    AgentReadParams, AgentRenameParams, AgentSendKeysParams, AgentStartParams, AgentTarget,
    AgentViewClearParams, AgentViewSetParams, AgentWaitParams, ClientWindowTitleSetParams,
    EmptyParams, EventsSubscribeParams, EventsWaitParams, IntegrationInstallParams,
    IntegrationUninstallParams, LayoutApplyParams, LayoutExportParams, LayoutSetSplitRatioParams,
    NotificationShowParams, PaneClearAgentAuthorityParams, PaneCurrentParams, PaneEdgesParams,
    PaneFocusDirectionParams, PaneGraphicsClearParams, PaneGraphicsDirectParams,
    PaneGraphicsSetParams, PaneGraphicsStreamParams, PaneInputSetParams, PaneLayoutParams,
    PaneListParams, PaneMoveParams, PaneNeighborParams, PaneProcessInfoParams, PaneReadParams,
    PaneReleaseAgentParams, PaneRenameParams, PaneReportAgentParams, PaneReportAgentSessionParams,
    PaneReportMetadataParams, PaneResizeParams, PaneSendInputParams, PaneSendKeysParams,
    PaneSendTextParams, PaneSplitParams, PaneSwapParams, PaneTarget, PaneWaitForOutputParams,
    PaneZoomParams, PingParams, ServerLiveHandoffParams, TabCreateParams, TabListParams,
    TabMoveParams, TabRenameParams, TabTarget, WorkspaceCreateParams, WorkspaceMoveBlockParams,
    WorkspaceMoveParams, WorkspaceRenameParams, WorkspaceReportMetadataParams, WorkspaceTarget,
    WorktreeCreateParams, WorktreeListParams, WorktreeOpenParams, WorktreeRemoveParams,
};
use serde::{Deserialize, Serialize};

mod response;
pub use response::{RpcErrorRef, RpcResponse, serialize_rpc_response};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// A command with its optional correlation identifier, flattened into one record; the
/// command family defaults to the workspace commands.
#[schemars(description = "", rename = "Request")]
pub struct Request<C = RpcCommand> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Id.
    #[schemars(description = "")]
    pub id: Option<String>,
    #[serde(flatten)]
    /// Command.
    #[schemars(description = "")]
    pub command: C,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type")]
/// Typed workspace commands.
#[schemars(description = "")]
pub enum RpcCommand {
    #[serde(rename = "ping")]
    /// Ping.
    #[schemars(description = "")]
    Ping(PingParams),
    #[serde(rename = "server.stop")]
    /// Server stop.
    #[schemars(description = "")]
    ServerStop(EmptyParams),
    #[serde(rename = "server.live_handoff")]
    /// Server live handoff.
    #[schemars(description = "")]
    ServerLiveHandoff(ServerLiveHandoffParams),
    #[serde(rename = "server.reload_config")]
    /// Server reload config.
    #[schemars(description = "")]
    ServerReloadConfig(EmptyParams),
    #[serde(rename = "server.agent_manifests")]
    /// Server agent manifests.
    #[schemars(description = "")]
    ServerAgentManifests(EmptyParams),
    #[serde(rename = "server.reload_agent_manifests")]
    /// Server reload agent manifests.
    #[schemars(description = "")]
    ServerReloadAgentManifests(EmptyParams),
    #[serde(rename = "notification.show")]
    /// Notification show.
    #[schemars(description = "")]
    NotificationShow(NotificationShowParams),
    #[serde(rename = "client.window_title.set")]
    /// Client window title set.
    #[schemars(description = "")]
    ClientWindowTitleSet(ClientWindowTitleSetParams),
    #[serde(rename = "client.window_title.clear")]
    /// Client window title clear.
    #[schemars(description = "")]
    ClientWindowTitleClear(EmptyParams),
    #[serde(rename = "session.snapshot")]
    /// Session snapshot.
    #[schemars(description = "")]
    SessionSnapshot(EmptyParams),
    #[serde(rename = "workspace.create")]
    /// Workspace create.
    #[schemars(description = "")]
    WorkspaceCreate(WorkspaceCreateParams),
    #[serde(rename = "workspace.list")]
    /// Workspace list.
    #[schemars(description = "")]
    WorkspaceList(EmptyParams),
    #[serde(rename = "workspace.get")]
    /// Workspace get.
    #[schemars(description = "")]
    WorkspaceGet(WorkspaceTarget),
    #[serde(rename = "workspace.focus")]
    /// Workspace focus.
    #[schemars(description = "")]
    WorkspaceFocus(WorkspaceTarget),
    #[serde(rename = "workspace.rename")]
    /// Workspace rename.
    #[schemars(description = "")]
    WorkspaceRename(WorkspaceRenameParams),
    #[serde(rename = "workspace.move")]
    /// Workspace move.
    #[schemars(description = "")]
    WorkspaceMove(WorkspaceMoveParams),
    #[serde(rename = "workspace.move_block")]
    /// Workspace move block.
    #[schemars(description = "")]
    WorkspaceMoveBlock(WorkspaceMoveBlockParams),
    #[serde(rename = "workspace.report_metadata")]
    /// Workspace report metadata.
    #[schemars(description = "")]
    WorkspaceReportMetadata(WorkspaceReportMetadataParams),
    #[serde(rename = "workspace.close")]
    /// Workspace close.
    #[schemars(description = "")]
    WorkspaceClose(WorkspaceTarget),
    #[serde(rename = "worktree.list")]
    /// Worktree list.
    #[schemars(description = "")]
    WorktreeList(WorktreeListParams),
    #[serde(rename = "worktree.create")]
    /// Worktree create.
    #[schemars(description = "")]
    WorktreeCreate(WorktreeCreateParams),
    #[serde(rename = "worktree.open")]
    /// Worktree open.
    #[schemars(description = "")]
    WorktreeOpen(WorktreeOpenParams),
    #[serde(rename = "worktree.remove")]
    /// Worktree remove.
    #[schemars(description = "")]
    WorktreeRemove(WorktreeRemoveParams),
    #[serde(rename = "tab.create")]
    /// Tab create.
    #[schemars(description = "")]
    TabCreate(TabCreateParams),
    #[serde(rename = "tab.list")]
    /// Tab list.
    #[schemars(description = "")]
    TabList(TabListParams),
    #[serde(rename = "tab.get")]
    /// Tab get.
    #[schemars(description = "")]
    TabGet(TabTarget),
    #[serde(rename = "tab.focus")]
    /// Tab focus.
    #[schemars(description = "")]
    TabFocus(TabTarget),
    #[serde(rename = "tab.rename")]
    /// Tab rename.
    #[schemars(description = "")]
    TabRename(TabRenameParams),
    #[serde(rename = "tab.move")]
    /// Tab move.
    #[schemars(description = "")]
    TabMove(TabMoveParams),
    #[serde(rename = "tab.close")]
    /// Tab close.
    #[schemars(description = "")]
    TabClose(TabTarget),
    #[serde(rename = "agent.list")]
    /// Agent list.
    #[schemars(description = "")]
    AgentList(EmptyParams),
    #[serde(rename = "agent.get")]
    /// Agent get.
    #[schemars(description = "")]
    AgentGet(AgentTarget),
    #[serde(rename = "agent.read")]
    /// Agent read.
    #[schemars(description = "")]
    AgentRead(AgentReadParams),
    #[serde(rename = "agent.explain")]
    /// Agent explain.
    #[schemars(description = "")]
    AgentExplain(AgentTarget),
    #[serde(rename = "agent.send_keys")]
    /// Agent send keys.
    #[schemars(description = "")]
    AgentSendKeys(AgentSendKeysParams),
    #[serde(rename = "agent.rename")]
    /// Agent rename.
    #[schemars(description = "")]
    AgentRename(AgentRenameParams),
    #[serde(rename = "agent.view.set")]
    /// Agent view set.
    #[schemars(description = "")]
    AgentViewSet(AgentViewSetParams),
    #[serde(rename = "agent.view.clear")]
    /// Agent view clear.
    #[schemars(description = "")]
    AgentViewClear(AgentViewClearParams),
    #[serde(rename = "agent.focus")]
    /// Agent focus.
    #[schemars(description = "")]
    AgentFocus(AgentTarget),
    #[serde(rename = "agent.start")]
    /// Agent start.
    #[schemars(description = "")]
    AgentStart(AgentStartParams),
    #[serde(rename = "agent.prompt")]
    /// Agent prompt.
    #[schemars(description = "")]
    AgentPrompt(AgentPromptParams),
    #[serde(rename = "agent.wait")]
    /// Agent wait.
    #[schemars(description = "")]
    AgentWait(AgentWaitParams),
    #[serde(rename = "pane.split")]
    /// Pane split.
    #[schemars(description = "")]
    PaneSplit(PaneSplitParams),
    #[serde(rename = "pane.swap")]
    /// Pane swap.
    #[schemars(description = "")]
    PaneSwap(PaneSwapParams),
    #[serde(rename = "pane.move")]
    /// Pane move.
    #[schemars(description = "")]
    PaneMove(PaneMoveParams),
    #[serde(rename = "pane.zoom")]
    /// Pane zoom.
    #[schemars(description = "")]
    PaneZoom(PaneZoomParams),
    #[serde(rename = "pane.layout")]
    /// Pane layout.
    #[schemars(description = "")]
    PaneLayout(PaneLayoutParams),
    #[serde(rename = "pane.process_info")]
    /// Pane process info.
    #[schemars(description = "")]
    PaneProcessInfo(PaneProcessInfoParams),
    #[serde(rename = "layout.export")]
    /// Layout export.
    #[schemars(description = "")]
    LayoutExport(LayoutExportParams),
    #[serde(rename = "layout.apply")]
    /// Layout apply.
    #[schemars(description = "")]
    LayoutApply(LayoutApplyParams),
    #[serde(rename = "layout.set_split_ratio")]
    /// Layout set split ratio.
    #[schemars(description = "")]
    LayoutSetSplitRatio(LayoutSetSplitRatioParams),
    #[serde(rename = "pane.neighbor")]
    /// Pane neighbor.
    #[schemars(description = "")]
    PaneNeighbor(PaneNeighborParams),
    #[serde(rename = "pane.edges")]
    /// Pane edges.
    #[schemars(description = "")]
    PaneEdges(PaneEdgesParams),
    #[serde(rename = "pane.focus_direction")]
    /// Pane focus direction.
    #[schemars(description = "")]
    PaneFocusDirection(PaneFocusDirectionParams),
    #[serde(rename = "pane.resize")]
    /// Pane resize.
    #[schemars(description = "")]
    PaneResize(PaneResizeParams),
    #[serde(rename = "pane.list")]
    /// Pane list.
    #[schemars(description = "")]
    PaneList(PaneListParams),
    #[serde(rename = "pane.current")]
    /// Pane current.
    #[schemars(description = "")]
    PaneCurrent(PaneCurrentParams),
    #[serde(rename = "pane.get")]
    /// Pane get.
    #[schemars(description = "")]
    PaneGet(PaneTarget),
    #[serde(rename = "pane.focus")]
    /// Pane focus.
    #[schemars(description = "")]
    PaneFocus(PaneTarget),
    #[serde(rename = "pane.input.set")]
    /// Pane input set.
    #[schemars(description = "")]
    PaneInputSet(PaneInputSetParams),
    #[serde(rename = "pane.rename")]
    /// Pane rename.
    #[schemars(description = "")]
    PaneRename(PaneRenameParams),
    #[serde(rename = "pane.send_text")]
    /// Pane send text.
    #[schemars(description = "")]
    PaneSendText(PaneSendTextParams),
    #[serde(rename = "pane.send_keys")]
    /// Pane send keys.
    #[schemars(description = "")]
    PaneSendKeys(PaneSendKeysParams),
    #[serde(rename = "pane.send_input")]
    /// Pane send input.
    #[schemars(description = "")]
    PaneSendInput(PaneSendInputParams),
    #[serde(rename = "pane.read")]
    /// Pane read.
    #[schemars(description = "")]
    PaneRead(PaneReadParams),
    #[serde(rename = "pane.graphics.set")]
    /// Pane graphics set.
    #[schemars(description = "")]
    PaneGraphicsSet(PaneGraphicsSetParams),
    #[serde(rename = "pane.graphics.clear")]
    /// Pane graphics clear.
    #[schemars(description = "")]
    PaneGraphicsClear(PaneGraphicsClearParams),
    #[serde(rename = "pane.graphics.info")]
    /// Pane graphics info.
    #[schemars(description = "")]
    PaneGraphicsInfo(PaneTarget),
    #[serde(rename = "pane.graphics.stream")]
    /// Pane graphics stream.
    #[schemars(description = "")]
    PaneGraphicsStream(PaneGraphicsStreamParams),
    #[serde(skip)]
    #[schemars(skip)]
    /// Pane graphics stream set.
    #[schemars(description = "")]
    PaneGraphicsStreamSet(PaneGraphicsSetParams),
    #[serde(skip)]
    #[schemars(skip)]
    /// Pane graphics stream direct.
    #[schemars(description = "")]
    PaneGraphicsStreamDirect(PaneGraphicsDirectParams),
    #[serde(skip)]
    #[schemars(skip)]
    /// Pane graphics stream open.
    #[schemars(description = "")]
    PaneGraphicsStreamOpen(PaneGraphicsStreamParams),
    #[serde(skip)]
    #[schemars(skip)]
    /// Pane graphics stream close.
    #[schemars(description = "")]
    PaneGraphicsStreamClose(PaneGraphicsStreamParams),
    #[serde(rename = "pane.report_agent")]
    /// Pane report agent.
    #[schemars(description = "")]
    PaneReportAgent(PaneReportAgentParams),
    #[serde(rename = "pane.report_agent_session")]
    /// Pane report agent session.
    #[schemars(description = "")]
    PaneReportAgentSession(PaneReportAgentSessionParams),
    #[serde(rename = "pane.report_metadata")]
    /// Pane report metadata.
    #[schemars(description = "")]
    PaneReportMetadata(PaneReportMetadataParams),
    #[serde(rename = "pane.clear_agent_authority")]
    /// Pane clear agent authority.
    #[schemars(description = "")]
    PaneClearAgentAuthority(PaneClearAgentAuthorityParams),
    #[serde(rename = "pane.release_agent")]
    /// Pane release agent.
    #[schemars(description = "")]
    PaneReleaseAgent(PaneReleaseAgentParams),
    #[serde(rename = "pane.close")]
    /// Pane close.
    #[schemars(description = "")]
    PaneClose(PaneTarget),
    #[serde(rename = "popup.close")]
    /// Popup close.
    #[schemars(description = "")]
    PopupClose(EmptyParams),
    #[serde(rename = "events.subscribe")]
    /// Events subscribe.
    #[schemars(description = "")]
    EventsSubscribe(EventsSubscribeParams),
    #[serde(rename = "events.wait")]
    /// Events wait.
    #[schemars(description = "")]
    EventsWait(EventsWaitParams),
    #[serde(rename = "pane.wait_for_output")]
    /// Pane wait for output.
    #[schemars(description = "")]
    PaneWaitForOutput(PaneWaitForOutputParams),
    #[serde(rename = "integration.install")]
    /// Integration install.
    #[schemars(description = "")]
    IntegrationInstall(IntegrationInstallParams),
    #[serde(rename = "integration.uninstall")]
    /// Integration uninstall.
    #[schemars(description = "")]
    IntegrationUninstall(IntegrationUninstallParams),
    #[serde(rename = "action.manifest.link")]
    /// Action link.
    #[schemars(description = "")]
    ActionManifestLink(ActionManifestLinkParams),
    #[serde(rename = "action.manifest.list")]
    /// Action list.
    #[schemars(description = "")]
    ActionManifestList(ActionManifestListParams),
    #[serde(rename = "action.manifest.unlink")]
    /// Action unlink.
    #[schemars(description = "")]
    ActionManifestUnlink(ActionManifestUnlinkParams),
    #[serde(rename = "action.manifest.enable")]
    /// Action enable.
    #[schemars(description = "")]
    ActionManifestEnable(ActionManifestSetEnabledParams),
    #[serde(rename = "action.manifest.disable")]
    /// Action disable.
    #[schemars(description = "")]
    ActionManifestDisable(ActionManifestSetEnabledParams),
    #[serde(rename = "action.list")]
    /// Action action list.
    #[schemars(description = "")]
    ActionList(ActionListParams),
    #[serde(rename = "action.invoke")]
    /// Action action invoke.
    #[schemars(description = "")]
    ActionInvoke(ActionInvokeParams),
    #[serde(rename = "action.log.list")]
    /// Action log list.
    #[schemars(description = "")]
    ActionLogList(ActionLogListParams),
    #[serde(rename = "action.pane.open")]
    /// Action pane open.
    #[schemars(description = "")]
    ActionPaneOpen(ActionPaneOpenParams),
    #[serde(rename = "action.pane.focus")]
    /// Action pane focus.
    #[schemars(description = "")]
    ActionPaneFocus(ActionPaneFocusParams),
    #[serde(rename = "action.pane.close")]
    /// Action pane close.
    #[schemars(description = "")]
    ActionPaneClose(ActionPaneCloseParams),
}
