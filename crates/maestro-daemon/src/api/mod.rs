//! The daemon API's replayed events, its requests, its socket client and the live daemon
//! status.

/// Shared bounded event history.
pub mod event_hub;

/// Synchronous requests to a caller-selected daemon endpoint.
#[cfg(unix)]
pub mod client;

/// The live status a daemon reports to a ping.
#[cfg(unix)]
pub mod status;

#[cfg(unix)]
pub use status::{RuntimeStatus, read_runtime_status_at};

/// The native local-socket operations the API server uses.
#[cfg(unix)]
pub use maestro_ipc::local_socket as socket;

use maestro_protocol::{
    rpc_types::RpcCommand,
    schema::{ErrorBody, ResponseResult},
};

/// One workspace command for the daemon loop, with the routes back to its connection.
pub struct RequestMessage {
    /// The decoded command.
    pub command: RpcCommand,
    /// Receives the command's typed result.
    pub respond_to: tokio::sync::oneshot::Sender<Result<ResponseResult, ErrorBody>>,
    /// Settles once the connection attempted to write the whole response, whatever the
    /// result; disconnects when the response is dropped before any attempt.
    pub response_write_complete: tokio::sync::oneshot::Receiver<()>,
}

/// Sends workspace commands to the daemon loop.
pub type ApiRequestSender = tokio::sync::mpsc::UnboundedSender<RequestMessage>;

/// Whether `command` changes what an attached view shows.
#[must_use]
pub fn request_changes_ui(command: &RpcCommand) -> bool {
    matches!(
        command,
        RpcCommand::ServerReloadConfig(_)
            | RpcCommand::ServerReloadAgentManifests(_)
            | RpcCommand::NotificationShow(_)
            | RpcCommand::WorkspaceCreate(_)
            | RpcCommand::WorkspaceFocus(_)
            | RpcCommand::WorkspaceRename(_)
            | RpcCommand::WorkspaceMove(_)
            | RpcCommand::WorkspaceMoveBlock(_)
            | RpcCommand::WorkspaceReportMetadata(_)
            | RpcCommand::WorkspaceClose(_)
            | RpcCommand::WorktreeCreate(_)
            | RpcCommand::WorktreeOpen(_)
            | RpcCommand::WorktreeRemove(_)
            | RpcCommand::TabCreate(_)
            | RpcCommand::TabFocus(_)
            | RpcCommand::TabRename(_)
            | RpcCommand::TabMove(_)
            | RpcCommand::TabClose(_)
            | RpcCommand::LayoutApply(_)
            | RpcCommand::LayoutSetSplitRatio(_)
            | RpcCommand::AgentRename(_)
            | RpcCommand::AgentViewSet(_)
            | RpcCommand::AgentViewClear(_)
            | RpcCommand::AgentFocus(_)
            | RpcCommand::AgentStart(_)
            | RpcCommand::AgentPrompt(_)
            | RpcCommand::AgentSendKeys(_)
            | RpcCommand::PaneSplit(_)
            | RpcCommand::PaneSwap(_)
            | RpcCommand::PaneMove(_)
            | RpcCommand::PaneZoom(_)
            | RpcCommand::PaneFocusDirection(_)
            | RpcCommand::PaneResize(_)
            | RpcCommand::PaneFocus(_)
            | RpcCommand::PaneInputSet(_)
            | RpcCommand::PaneRename(_)
            | RpcCommand::PaneGraphicsSet(_)
            | RpcCommand::PaneGraphicsClear(_)
            | RpcCommand::PaneGraphicsStream(_)
            | RpcCommand::PaneGraphicsStreamSet(_)
            | RpcCommand::PaneGraphicsStreamDirect(_)
            | RpcCommand::PaneGraphicsStreamOpen(_)
            | RpcCommand::PaneGraphicsStreamClose(_)
            | RpcCommand::PaneReportAgent(_)
            | RpcCommand::PaneReportAgentSession(_)
            | RpcCommand::PaneReportMetadata(_)
            | RpcCommand::PaneClearAgentAuthority(_)
            | RpcCommand::PaneReleaseAgent(_)
            | RpcCommand::PaneClose(_)
            | RpcCommand::PopupClose(_)
            | RpcCommand::ActionManifestUnlink(_)
            | RpcCommand::ActionManifestDisable(_)
            | RpcCommand::ActionInvoke(_)
            | RpcCommand::ActionPaneOpen(_)
            | RpcCommand::ActionPaneFocus(_)
            | RpcCommand::ActionPaneClose(_)
    )
}
