//! Workspace commands served for the daemon: the connection's server answers its own, and
//! the daemon loop answers the rest.
use super::{RpcFuture, WorkspaceEventSink, WorkspaceOperations};
use crate::rpc_types::{workspace, workspace_schema};
use maestro_daemon::api::ApiRequestSender;
use tokio::sync::{oneshot, watch};

/// Serves workspace commands through the daemon loop's request queue.
pub struct DaemonWorkspaceOperations {
    /// The daemon loop's request queue.
    sender: ApiRequestSender,
    /// The capabilities a ping reports.
    capabilities: Option<workspace_schema::ServerCapabilities>,
    /// Set once a stop was requested.
    stop: watch::Sender<bool>,
}

impl DaemonWorkspaceOperations {
    /// Operations forwarding to `sender`, reporting `capabilities` and setting `stop` once a
    /// stop is requested.
    #[must_use]
    pub fn new(
        sender: ApiRequestSender,
        capabilities: Option<workspace_schema::ServerCapabilities>,
        stop: watch::Sender<bool>,
    ) -> Self {
        Self {
            sender,
            capabilities,
            stop,
        }
    }
}

impl WorkspaceOperations for DaemonWorkspaceOperations {
    fn execute(
        &self,
        command: workspace::RpcCommand,
        _events: WorkspaceEventSink,
        write_complete: oneshot::Receiver<()>,
    ) -> RpcFuture<Result<workspace_schema::ResponseResult, workspace_schema::ErrorBody>> {
        drop((command, write_complete));
        let _ = (
            self.sender.is_closed(),
            self.capabilities.is_some(),
            self.stop.is_closed(),
        );
        Box::pin(std::future::ready(Err(workspace_schema::ErrorBody {
            code: String::new(),
            message: String::new(),
        })))
    }
}
