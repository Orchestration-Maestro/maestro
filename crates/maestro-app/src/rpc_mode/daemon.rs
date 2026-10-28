//! Workspace commands served for the daemon: the connection's server answers a ping and a
//! stop itself, and the daemon loop answers every other command.
use super::{RpcFuture, WorkspaceEventSink, WorkspaceOperations};
use crate::rpc_types::{workspace, workspace_schema};
use maestro_daemon::api::{ApiRequestSender, RequestMessage};
use tokio::sync::{oneshot, watch};

/// A command's typed result.
type CommandResult = Result<workspace_schema::ResponseResult, workspace_schema::ErrorBody>;

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

    /// Queues `command` for the daemon loop and waits for its answer, however long it takes.
    fn dispatch(
        &self,
        command: workspace::RpcCommand,
        write_complete: oneshot::Receiver<()>,
    ) -> RpcFuture<CommandResult> {
        let (respond_to, response) = oneshot::channel();
        let request = RequestMessage {
            command,
            respond_to,
            response_write_complete: write_complete,
        };
        if let Err(error) = self.sender.send(request) {
            let failed = unavailable(format!("failed to dispatch request: {error}"));
            return Box::pin(std::future::ready(failed));
        }
        Box::pin(async move {
            response.await.unwrap_or_else(|_| {
                unavailable("request handling failed: receiving on a closed channel".into())
            })
        })
    }
}

impl WorkspaceOperations for DaemonWorkspaceOperations {
    /// Answers a ping and a stop without the daemon loop; once stopped, refuses every other
    /// command, which otherwise waits on the loop.
    fn execute(
        &self,
        command: workspace::RpcCommand,
        _events: WorkspaceEventSink,
        write_complete: oneshot::Receiver<()>,
    ) -> RpcFuture<CommandResult> {
        let answered = match command {
            workspace::RpcCommand::Ping(_) => Ok(workspace_schema::ResponseResult::Pong {
                version: maestro_protocol::build_info::version(),
                protocol: maestro_protocol::PROTOCOL_VERSION,
                capabilities: self.capabilities.clone(),
            }),
            workspace::RpcCommand::ServerStop(_) => {
                self.stop.send_replace(true);
                Ok(workspace_schema::ResponseResult::Ok {})
            }
            _ if *self.stop.borrow() => unavailable("server is shutting down".into()),
            command => return self.dispatch(command, write_complete),
        };
        Box::pin(std::future::ready(answered))
    }
}

/// The failure of a command the daemon cannot take.
fn unavailable(message: String) -> CommandResult {
    Err(workspace_schema::ErrorBody {
        code: "server_unavailable".into(),
        message,
    })
}
