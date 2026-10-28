//! The live daemon status, read from a ping reply rather than a status file.

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

/// Pings the daemon at `socket_path` within `timeout`.
///
/// # Errors
/// Not yet implemented: never fails.
pub fn read_runtime_status_at(
    socket_path: &Path,
    timeout: Duration,
) -> io::Result<Option<RuntimeStatus>> {
    let _ = (socket_path, timeout);
    Ok(None)
}
