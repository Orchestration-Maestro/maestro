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
