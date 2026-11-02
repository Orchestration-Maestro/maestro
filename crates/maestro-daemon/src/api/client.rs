//! Reusable synchronous client for a retained native daemon endpoint.

use crate::api::status::RuntimeStatus;
use crate::instances::Selection;
use maestro_ipc::local_socket::{LocalStream, LocalStreamExt, connect_local_stream};
use maestro_json::jsonl::serialize_native_json_line;
use maestro_protocol::{
    rpc_types::{Request, RpcCommand, RpcResponse},
    schema::{PingParams, ResponseResult},
};
use std::{
    fmt,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::Duration,
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
    /// The connection has no timeout of its own.
    ///
    /// # Errors
    /// Returns the original connection, write, read or JSON error, or an empty-response
    /// error when the peer supplies no nonblank line.
    pub fn request_value(&self, request: &Request) -> Result<serde_json::Value, ApiClientError> {
        exchange(connect_local_stream(self.target.path())?, request)
    }

    /// Sends one request and decodes the shared response.
    ///
    /// # Errors
    /// Returns a [`DaemonClient::request_value`] failure, a decoding failure, or the
    /// daemon's failure with its correlation and command.
    pub fn request(&self, request: &Request) -> Result<RpcResponse, ApiClientError> {
        let response: RpcResponse = serde_json::from_value(self.request_value(request)?)?;
        match response.result {
            Ok(_) => Ok(response),
            Err(_) => Err(ApiClientError::ErrorResponse(Box::new(response))),
        }
    }

    /// Sends one request with `timeout` as the native send and receive timeouts of its
    /// connection.
    ///
    /// # Errors
    /// Returns the native failure to set either timeout, such as invalid input for a zero
    /// duration, a timed-out write or read, or any [`DaemonClient::request_value`] failure.
    pub fn request_value_with_timeout(
        &self,
        request: &Request,
        timeout: Duration,
    ) -> Result<serde_json::Value, ApiClientError> {
        let stream = connect_local_stream(self.target.path())?;
        stream.set_send_timeout(Some(timeout))?;
        stream.set_recv_timeout(Some(timeout))?;
        exchange(stream, request)
    }

    /// Reads the daemon's live status from a ping.
    ///
    /// # Errors
    /// Returns a [`DaemonClient::request`] failure, or an unexpected-result error when the
    /// success is not a pong.
    pub fn status(&self) -> Result<RuntimeStatus, ApiClientError> {
        pong(self.request(&ping("api-client:status"))?)
    }
}

/// A ping request with the supplied correlation.
pub(crate) fn ping(id: &str) -> Request {
    Request {
        id: Some(id.into()),
        command: RpcCommand::Ping(PingParams::default()),
    }
}

/// Reads the live status from a decoded ping response.
///
/// # Errors
/// Returns the daemon's failure, or names any success other than a pong.
pub(crate) fn pong(response: RpcResponse) -> Result<RuntimeStatus, ApiClientError> {
    match response.result {
        Ok(ResponseResult::Pong {
            version,
            protocol,
            capabilities,
        }) => Ok(RuntimeStatus {
            version: Some(version),
            protocol: Some(protocol),
            capabilities,
        }),
        Ok(result) => Err(ApiClientError::UnexpectedResult(format!("{result:?}"))),
        Err(_) => Err(ApiClientError::ErrorResponse(Box::new(response))),
    }
}

/// Frames one request as a canonical line with exact integers.
///
/// # Errors
/// Returns the request's encoding failure.
pub(crate) fn request_line(request: &Request) -> Result<String, serde_json::Error> {
    serialize_native_json_line(&serde_json::to_value(request)?)
}

/// Writes one request line on `stream` and reads one nonblank JSON line back.
fn exchange(
    mut stream: LocalStream,
    request: &Request,
) -> Result<serde_json::Value, ApiClientError> {
    stream.write_all(request_line(request)?.as_bytes())?;
    stream.flush()?;
    let mut line = String::new();
    let read = BufReader::new(stream).read_line(&mut line)?;
    if read == 0 || line.trim().is_empty() {
        return Err(ApiClientError::EmptyResponse);
    }
    Ok(serde_json::from_str(&line)?)
}

/// A native client transport or response decoding failure.
#[derive(Debug)]
pub enum ApiClientError {
    /// The original native operation failed.
    Io(io::Error),
    /// Request encoding or response decoding failed.
    Json(serde_json::Error),
    /// The daemon answered with a failure, kept with its correlation and command.
    ErrorResponse(Box<RpcResponse>),
    /// The peer sent no nonblank response line.
    EmptyResponse,
    /// A success carried data other than the request's own result.
    UnexpectedResult(String),
}

impl fmt::Display for ApiClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Json(error) => error.fmt(f),
            Self::ErrorResponse(response) => f.write_str(
                response
                    .result
                    .as_ref()
                    .err()
                    .map_or("", |error| error.message.as_str()),
            ),
            Self::EmptyResponse => f.write_str("empty api response"),
            Self::UnexpectedResult(result) => write!(f, "unexpected api result: {result}"),
        }
    }
}

impl std::error::Error for ApiClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::ErrorResponse(_) | Self::EmptyResponse | Self::UnexpectedResult(_) => None,
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
