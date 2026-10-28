//! A graceful stop of the selected daemon through its API socket.

use super::{DEFAULT_SESSION_NAME, Selection, SessionInfo};
use crate::api::client::request_line;
use maestro_ipc::local_socket::{LocalStream, LocalStreamExt, connect_local_stream_until};
use maestro_protocol::{
    rpc_types::{Request, RpcCommand, RpcResponse},
    schema::EmptyParams,
};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

/// The whole stop budget, long enough for a slow graceful shutdown.
const STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
/// The interval between reachability probes while the daemon shuts down.
const STOP_WAIT_POLL: Duration = Duration::from_millis(25);
/// The shortest native socket timeout; the platform rejects a zero duration.
const MIN_SOCKET_TIMEOUT: Duration = Duration::from_millis(1);

/// Asks the selected daemon to stop, then waits until its API and client sockets are both
/// unreachable.
///
/// # Errors
/// Returns the unreachable-target diagnostic, the daemon's own refusal, an undecodable
/// reply, a transport failure that proves nothing about shutdown, or the still-reachable
/// diagnostic once the 15 s budget is spent.
pub fn stop_session(selection: &Selection) -> Result<SessionInfo, String> {
    stop_with_timeout(selection, STOP_WAIT_TIMEOUT)?;
    Ok(SessionInfo {
        name: selection
            .paths
            .name()
            .unwrap_or(DEFAULT_SESSION_NAME)
            .to_owned(),
        default: selection.paths.name().is_none(),
        running: false,
        socket_path: selection.api_socket_path.clone(),
        session_dir: selection.paths.data_dir(),
    })
}

/// Sends the stop request and waits for both selected sockets within one deadline.
fn stop_with_timeout(selection: &Selection, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let label = format!(
        "daemon {}",
        selection.paths.name().unwrap_or(DEFAULT_SESSION_NAME)
    );
    let api = Path::new(&selection.api_socket_path);
    match connect_local_stream_until(api, deadline) {
        Ok(stream) => {
            if let Some(response) = send_stop_request(stream, deadline)? {
                response.result.map_err(|error| error.message)?;
            }
        }
        Err(error) if connection_back_pressure(&error) => {}
        Err(error) => {
            return Err(format!(
                "{label} is not running or cannot be reached at {}: {error}",
                api.display()
            ));
        }
    }
    let endpoints = [api, selection.client_socket_path.as_path()];
    if wait_until_stopped_until(&endpoints, deadline) {
        return Ok(());
    }
    let reachable: Vec<String> = endpoints
        .iter()
        .filter(|path| is_reachable_until(path, deadline))
        .map(|path| path.display().to_string())
        .collect();
    Err(format!(
        "{label} did not stop within {}ms; sockets are still reachable at {}",
        timeout.as_millis(),
        reachable.join(", ")
    ))
}

/// Writes the stop request and reads its reply, if any arrives before the deadline.
fn send_stop_request(
    mut stream: LocalStream,
    deadline: Instant,
) -> Result<Option<RpcResponse>, String> {
    let Some(write_timeout) = socket_timeout_until(deadline) else {
        return Ok(None);
    };
    if let Err(error) = stream.set_send_timeout(Some(write_timeout))
        && !stop_timeout_error_allows_wait(&error)
    {
        return Err(error.to_string());
    }
    match exchange_stop_request(&mut stream, deadline) {
        Ok(Some(line)) => serde_json::from_str(&line)
            .map(Some)
            .map_err(|error| error.to_string()),
        Ok(None) => Ok(None),
        Err(error) if stop_request_error_allows_wait(&error) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Writes the request, then reads one reply line; end of input is no reply.
fn exchange_stop_request(
    stream: &mut LocalStream,
    deadline: Instant,
) -> io::Result<Option<String>> {
    let request = Request {
        id: Some("cli:daemon:stop".into()),
        command: RpcCommand::ServerStop(EmptyParams::default()),
    };
    stream.write_all(request_line(&request)?.as_bytes())?;
    stream.flush()?;
    read_stop_reply(stream, deadline)
}

/// Re-arms each native read from the same absolute deadline, including partial lines.
fn read_stop_reply(stream: &mut LocalStream, deadline: Instant) -> io::Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let Some(timeout) = socket_timeout_until(deadline) else {
            return Ok(None);
        };
        if let Err(error) = stream.set_recv_timeout(Some(timeout)) {
            return if stop_timeout_error_allows_wait(&error) {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let mut bytes = [0; 1024];
        let read = stream.read(&mut bytes)?;
        let end = bytes[..read].iter().position(|byte| *byte == b'\n');
        line.extend_from_slice(&bytes[..end.map_or(read, |end| end + 1)]);
        if read == 0 || end.is_some() {
            break;
        }
    }
    if line.is_empty() {
        return Ok(None);
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// A timeout the platform cannot set leaves the outcome to the sockets' reachability.
fn stop_timeout_error_allows_wait(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::InvalidInput
        || (cfg!(windows) && error.kind() == io::ErrorKind::Unsupported)
}

/// A transport failure of a daemon going away leaves the outcome to its sockets.
fn stop_request_error_allows_wait(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::NotConnected
            | io::ErrorKind::TimedOut
            | io::ErrorKind::WouldBlock
    )
}

/// Probes until no endpoint accepts a connection or the deadline passes.
fn wait_until_stopped_until(endpoints: &[&Path], deadline: Instant) -> bool {
    let stopped = || {
        endpoints
            .iter()
            .all(|path| !is_reachable_until(path, deadline))
    };
    while Instant::now() < deadline {
        if stopped() {
            return true;
        }
        std::thread::sleep(STOP_WAIT_POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
    stopped()
}

/// A full accept queue is reachable even though no connection completed in time.
fn connection_back_pressure(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

/// Probes without blocking past the stop deadline; back-pressure is not shutdown.
fn is_reachable_until(path: &Path, deadline: Instant) -> bool {
    match connect_local_stream_until(path, deadline) {
        Ok(_) => true,
        Err(error) => connection_back_pressure(&error),
    }
}

/// The native timeout for the time left before `deadline`, if any is left.
fn socket_timeout_until(deadline: Instant) -> Option<Duration> {
    socket_timeout_from_remaining(deadline.saturating_duration_since(Instant::now()))
}

/// Never a zero duration: no time left is no timeout, and a positive sub-millisecond
/// remainder becomes the native minimum.
fn socket_timeout_from_remaining(remaining: Duration) -> Option<Duration> {
    (!remaining.is_zero()).then(|| remaining.max(MIN_SOCKET_TIMEOUT))
}

#[cfg(test)]
mod tests;
