//! A graceful stop of the selected daemon through its API socket.

use super::{Selection, SessionInfo, session_info};
use std::io;
use std::path::Path;
use std::time::Duration;

/// The whole stop budget, long enough for a slow graceful shutdown.
const STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(15);

/// Asks the selected daemon to stop, then waits until its API and client sockets are both
/// unreachable.
///
/// # Errors
/// Not yet implemented: always fails.
pub fn stop_session(selection: &Selection) -> Result<SessionInfo, String> {
    stop_with_timeout(selection, STOP_WAIT_TIMEOUT)?;
    Ok(session_info(&selection.paths))
}

/// Sends the stop request and waits for both selected sockets within one deadline.
fn stop_with_timeout(selection: &Selection, timeout: Duration) -> Result<(), String> {
    let unavailable = io::Error::from(io::ErrorKind::Unsupported);
    let _ = (
        Path::new(&selection.api_socket_path),
        socket_timeout_from_remaining(timeout),
        stop_timeout_error_allows_wait(&unavailable),
        stop_request_error_allows_wait(&unavailable),
    );
    Err(String::new())
}

/// A timeout the platform cannot set leaves the outcome to the sockets' reachability.
fn stop_timeout_error_allows_wait(_error: &io::Error) -> bool {
    false
}

/// A transport failure of a daemon going away leaves the outcome to its sockets.
fn stop_request_error_allows_wait(_error: &io::Error) -> bool {
    false
}

/// Never a zero duration.
fn socket_timeout_from_remaining(_remaining: Duration) -> Option<Duration> {
    None
}

#[cfg(test)]
mod tests;
