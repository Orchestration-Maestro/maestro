//! Reusable synchronous client for a retained native daemon endpoint.

use crate::instances::Selection;
use maestro_ipc::local_socket::connect_local_stream;
use maestro_json::jsonl::serialize_json_line;
use maestro_protocol::rpc_types::Request;
use std::{
    fmt,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

/// The resolved API endpoint retained by a client.
#[derive(Clone, Debug)]
pub enum ConnectionTarget {
    /// An owned daemon selection, including its resolved API path.
    LocalSession(Selection),
    /// An explicit native socket path, kept as supplied.
    SocketPath(PathBuf),
}

impl ConnectionTarget {
    /// Borrows the selected path without normalizing or resolving it again.
    fn path(&self) -> &Path {
        match self {
            Self::LocalSession(selection) => Path::new(&selection.api_socket_path),
            Self::SocketPath(path) => path,
        }
    }
}

/// A reusable client that opens one connection for each request.
#[derive(Clone, Debug)]
pub struct DaemonClient {
    /// The endpoint selected by the caller.
    target: ConnectionTarget,
}

impl DaemonClient {
    /// Retains an already-resolved daemon selection.
    #[must_use]
    pub fn local(selection: Selection) -> Self {
        Self::for_target(ConnectionTarget::LocalSession(selection))
    }

    /// Retains the caller's selected endpoint.
    #[must_use]
    pub fn for_target(target: ConnectionTarget) -> Self {
        Self { target }
    }

    /// Returns the retained native socket spelling.
    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        self.target.path().to_path_buf()
    }

    /// Writes one shared request frame and reads one raw JSON response.
    ///
    /// # Errors
    /// Returns the original connection, write, read or JSON error, or an empty-response
    /// error when the peer supplies no nonblank line.
    pub fn request_value(&self, request: &Request) -> Result<serde_json::Value, ApiClientError> {
        let mut stream = connect_local_stream(self.target.path())?;
        let line = serialize_json_line(&serde_json::to_value(request)?)?;
        stream.write_all(line.as_bytes())?;
        stream.flush()?;
        let mut line = String::new();
        let read = BufReader::new(stream).read_line(&mut line)?;
        if read == 0 || line.trim().is_empty() {
            return Err(ApiClientError::EmptyResponse);
        }
        Ok(serde_json::from_str(&line)?)
    }
}

/// A native client transport or response decoding failure.
#[derive(Debug)]
pub enum ApiClientError {
    /// The original native operation failed.
    Io(io::Error),
    /// Request encoding or response decoding failed.
    Json(serde_json::Error),
    /// The peer sent no nonblank response line.
    EmptyResponse,
}

impl fmt::Display for ApiClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Json(error) => error.fmt(f),
            Self::EmptyResponse => f.write_str("empty api response"),
        }
    }
}

impl std::error::Error for ApiClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::EmptyResponse => None,
        }
    }
}

impl From<io::Error> for ApiClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ApiClientError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
