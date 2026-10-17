//! Mod protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};

/// Agents.
pub mod agents;
/// Common.
pub mod common;
/// Event payload records.
mod event_data;
/// Events.
pub mod events;
/// Integrations.
pub mod integrations;
/// Pane graphics records.
mod pane_graphics;
/// Pane results.
mod pane_results;
/// Panes.
pub mod panes;
/// Plugins.
pub mod plugins;
/// Response.
pub mod response;
/// Server.
pub mod server;
/// Session.
pub mod session;
/// Tabs.
pub mod tabs;
/// Workspaces.
pub mod workspaces;
/// Worktrees.
pub mod worktrees;

pub use agents::*;
pub use common::*;
pub use events::*;
pub use integrations::*;
pub use panes::*;
pub use plugins::*;
pub use response::*;
pub use server::*;
pub use session::*;
pub use tabs::*;
pub use workspaces::*;
pub use worktrees::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
/// Request record.
#[schemars(description = "")]
pub struct Request {
    /// Id.
    #[schemars(description = "")]
    pub id: String,
    #[serde(flatten)]
    /// Method.
    #[schemars(description = "")]
    pub method: Method,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "method", content = "params")]
/// Method record.
#[schemars(description = "")]
pub enum Method {
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
    #[schemars(skip)]
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
    #[serde(rename = "plugin.link")]
    /// Plugin link.
    #[schemars(description = "")]
    PluginLink(PluginLinkParams),
    #[serde(rename = "plugin.list")]
    /// Plugin list.
    #[schemars(description = "")]
    PluginList(PluginListParams),
    #[serde(rename = "plugin.unlink")]
    /// Plugin unlink.
    #[schemars(description = "")]
    PluginUnlink(PluginUnlinkParams),
    #[serde(rename = "plugin.enable")]
    /// Plugin enable.
    #[schemars(description = "")]
    PluginEnable(PluginSetEnabledParams),
    #[serde(rename = "plugin.disable")]
    /// Plugin disable.
    #[schemars(description = "")]
    PluginDisable(PluginSetEnabledParams),
    #[serde(rename = "plugin.action.list")]
    /// Plugin action list.
    #[schemars(description = "")]
    PluginActionList(PluginActionListParams),
    #[serde(rename = "plugin.action.invoke")]
    /// Plugin action invoke.
    #[schemars(description = "")]
    PluginActionInvoke(PluginActionInvokeParams),
    #[serde(rename = "plugin.log.list")]
    /// Plugin log list.
    #[schemars(description = "")]
    PluginLogList(PluginLogListParams),
    #[serde(rename = "plugin.pane.open")]
    /// Plugin pane open.
    #[schemars(description = "")]
    PluginPaneOpen(PluginPaneOpenParams),
    #[serde(rename = "plugin.pane.focus")]
    /// Plugin pane focus.
    #[schemars(description = "")]
    PluginPaneFocus(PluginPaneFocusParams),
    #[serde(rename = "plugin.pane.close")]
    /// Plugin pane close.
    #[schemars(description = "")]
    PluginPaneClose(PluginPaneCloseParams),
}

/// Retain compact string enums when Rust variant documentation is present.
pub(crate) fn string_enum(schema: &mut schemars::Schema) {
    let is_string_enum = schema
        .get("oneOf")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|variants| {
            variants.iter().all(|variant| {
                variant.as_object().is_some_and(|object| {
                    object.len() == 2
                        && object.get("type").and_then(serde_json::Value::as_str) == Some("string")
                        && object
                            .get("const")
                            .is_some_and(serde_json::Value::is_string)
                })
            })
        });
    if !is_string_enum {
        return;
    }
    if let Some(serde_json::Value::Array(variants)) = schema
        .as_object_mut()
        .and_then(|object| object.shift_remove("oneOf"))
    {
        let values = variants
            .into_iter()
            .filter_map(|mut variant| variant.as_object_mut()?.shift_remove("const"))
            .collect();
        schema.insert("type".into(), serde_json::json!("string"));
        schema.insert("enum".into(), serde_json::Value::Array(values));
    }
}
