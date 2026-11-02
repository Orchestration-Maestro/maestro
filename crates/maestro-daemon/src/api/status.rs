//! The live daemon status, read from a ping reply rather than a status file.

use crate::api::client::{ApiClientError, ConnectionTarget, DaemonClient, ping, pong};
use maestro_protocol::schema::ServerCapabilities;
use std::{io, path::Path, time::Duration};

/// The product, protocol and capabilities a running daemon reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeStatus {
    /// The daemon's product version.
    pub version: Option<String>,
    /// The daemon's protocol version.
    pub protocol: Option<u32>,
    /// What the daemon can do beyond the shared protocol.
    pub capabilities: Option<ServerCapabilities>,
}

/// Pings the daemon at `socket_path`, with `timeout` on each native socket operation.
///
/// An absent socket, a refused connection, a vanished socket and a daemon that does not
/// answer in time all mean no status.
///
/// # Errors
/// Returns the daemon's failure, an undecodable reply, a success other than a pong, or any
/// other native failure, each with its original diagnostic.
pub fn read_runtime_status_at(
    socket_path: &Path,
    timeout: Duration,
) -> io::Result<Option<RuntimeStatus>> {
    if !socket_path.try_exists()? {
        return Ok(None);
    }
    let client = DaemonClient::for_target(ConnectionTarget::SocketPath(socket_path.to_owned()));
    let status = client
        .request_value_with_timeout(&ping("runtime:status"), timeout)
        .and_then(|value| pong(serde_json::from_value(value)?));
    match status {
        Ok(status) => Ok(Some(status)),
        Err(ApiClientError::Io(error)) if unreachable(&error) => Ok(None),
        Err(ApiClientError::UnexpectedResult(result)) => Err(io::Error::other(format!(
            "server status request returned unexpected result: {result}"
        ))),
        Err(error) => Err(io::Error::other(error)),
    }
}

/// No daemon answers: the connection is refused, the socket vanished, or the reply did not
/// arrive in time, which a Unix receive timeout reports as would-block.
fn unreachable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::NotFound
            | io::ErrorKind::TimedOut
            | io::ErrorKind::WouldBlock
    )
}
