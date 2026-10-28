//! The API socket: its private listener, the connections it accepts and the removal of the
//! socket it bound.
use maestro_app::LineTooLarge;
use maestro_app::daemon::api::socket::{
    AsyncLocalListener, AsyncLocalListenerExt, AsyncLocalStream, SocketFileIdentity,
    bind_async_local_listener, is_connection_closed_error, prepare_socket_path,
    remove_socket_file_if_owned, restrict_socket_permissions, socket_file_identity,
    split_async_stream,
};
use maestro_app::rpc_mode::{
    JsonlLineReader, RpcInput, RpcLineReader, RpcOutput, WorkspaceOperations,
};
use maestro_app::{RpcModeOptions, run_rpc_connection};
use std::path::{Path, PathBuf};
use std::{convert::Infallible, future::Future, io, pin::Pin, rc::Rc, time::Duration};
use tokio::io::AsyncRead;
use tokio::time::Instant;

/// The owner-only mode of the API socket.
const SOCKET_PERMISSION_MODE: u32 = 0o600;

/// How long a connection's first line may take.
const INITIAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a response write may make no progress.
const STREAM_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// The largest request line, in raw bytes before its LF.
const MAX_REQUEST_BYTES: usize = 1024 * 1024;

/// A bound API socket, removed when dropped unless another file replaced it.
pub(super) struct Server {
    /// The listener.
    listener: AsyncLocalListener,
    /// The socket's path.
    path: PathBuf,
    /// The socket file this server bound.
    identity: SocketFileIdentity,
}

impl Server {
    /// Binds a socket at `path` that only its owner may use, after removing a stale one.
    pub(super) fn bind(path: &Path) -> io::Result<Self> {
        prepare_socket_path(path, |path| {
            format!(
                "maestro is already running (socket busy at {})",
                path.display()
            )
        })?;
        let listener = bind_async_local_listener(path)?;
        restrict_socket_permissions(path, SOCKET_PERMISSION_MODE)?;
        let identity = socket_file_identity(path)?;
        tracing::info!(path = %path.display(), "api server listening");
        Ok(Self {
            listener,
            path: path.to_path_buf(),
            identity,
        })
    }

    /// Serves every accepted connection through `operations` until dropped. A failed accept
    /// ends accepting; the open connections go on.
    pub(super) async fn serve(&self, operations: Rc<dyn WorkspaceOperations>) -> Infallible {
        let mut connections = tokio::task::JoinSet::new();
        let failure = loop {
            let stream = match self.listener.accept().await {
                Ok(stream) => stream,
                Err(failure) => break failure,
            };
            let deadline = Instant::now() + INITIAL_REQUEST_TIMEOUT;
            while connections.try_join_next().is_some() {}
            connections.spawn_local(serve_connection(stream, Rc::clone(&operations), deadline));
        };
        tracing::error!(err = %failure, "api listener accept failed");
        std::future::pending().await
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Err(error) = remove_socket_file_if_owned(&self.path, &self.identity)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(
                path = %self.path.display(),
                err = %error,
                "failed to remove api socket on shutdown"
            );
        }
    }
}

/// Serves one connection until it ends; a failure other than the peer closing is logged.
async fn serve_connection(
    stream: AsyncLocalStream,
    operations: Rc<dyn WorkspaceOperations>,
    deadline: Instant,
) {
    let (input, output) = split_async_stream(stream);
    let options = RpcModeOptions {
        workspace: Some(operations),
        session: None,
    };
    let output = RpcOutput::new(output, Some(STREAM_WRITE_TIMEOUT));
    let served =
        run_rpc_connection(Box::new(Admission::new(input, deadline)), output, options).await;
    if let Err(error) = served
        && !is_connection_closed_error(&error)
    {
        tracing::warn!(err = %error, "api connection failed");
    }
}

/// A connection's lines: the first must complete within its deadline and a blank first line
/// ends the input; every line is trimmed before it is parsed.
pub(super) struct Admission<R> {
    /// The connection's strictly decoded lines.
    lines: JsonlLineReader<R>,
    /// The first line's deadline, until that line arrived.
    deadline: Option<Instant>,
}

impl<R> Admission<R> {
    /// Admits a connection reading from `reader` with its acceptance-based `deadline`.
    pub(super) fn new(reader: R, deadline: Instant) -> Self {
        Self {
            lines: JsonlLineReader::strict(reader, MAX_REQUEST_BYTES),
            deadline: Some(deadline),
        }
    }
}

impl<R: AsyncRead + Unpin> Admission<R> {
    /// The next trimmed line, the end of the input, or the first line's timeout.
    async fn next(&mut self) -> io::Result<RpcInput> {
        let next = match self.deadline {
            Some(deadline) => tokio::time::timeout_at(deadline, self.lines.next_line())
                .await
                .unwrap_or_else(|_| {
                    let expired = "timed out reading api request";
                    Err(io::Error::new(io::ErrorKind::TimedOut, expired))
                }),
            None => self.lines.next_line().await,
        };
        match next.map_err(oversized)? {
            RpcInput::Line(line) => {
                let line = line.trim();
                if self.deadline.take().is_some() && line.is_empty() {
                    return Ok(RpcInput::End { tail: None });
                }
                Ok(RpcInput::Line(line.to_owned()))
            }
            end @ RpcInput::End { .. } => Ok(end),
        }
    }
}

impl<R: AsyncRead + Unpin> RpcLineReader for Admission<R> {
    fn next_line(&mut self) -> Pin<Box<dyn Future<Output = io::Result<RpcInput>> + '_>> {
        Box::pin(self.next())
    }

    fn detach(&mut self) {
        self.lines.detach();
    }
}

/// Names a line over the limit as an oversized request; other failures stay as they are.
fn oversized(error: io::Error) -> io::Error {
    if error
        .get_ref()
        .is_some_and(<dyn std::error::Error + Send + Sync>::is::<LineTooLarge>)
    {
        return io::Error::new(io::ErrorKind::InvalidData, "api request line is too large");
    }
    error
}

#[cfg(test)]
mod tests;
