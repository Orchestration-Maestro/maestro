//! Helpers shared by the programs a test re-runs as controlled children and by the
//! parents that read their output.

use std::io;
use std::net::TcpStream;

use crate::HARNESS_START;

/// The standard output a child produces when it writes `body`.
#[must_use]
pub fn harness(body: &[u8]) -> Vec<u8> {
    [HARNESS_START, body].concat()
}

/// `output` without the harness start bytes of every process that wrote to it.
#[must_use]
pub fn without_harness(output: &[u8]) -> Vec<u8> {
    output.to_vec()
}

/// The text of variable `name`.
///
/// # Errors
///
/// Fails when the variable is unset or not text.
pub fn variable(name: &str) -> io::Result<String> {
    Err(io::Error::other(name.to_owned()))
}

/// The port number variable `name` holds.
///
/// # Errors
///
/// Fails when the variable is missing or not a port number.
pub fn port(name: &str) -> io::Result<u16> {
    Err(io::Error::other(name.to_owned()))
}

/// Writes `bytes` to standard output and flushes them.
///
/// # Errors
///
/// Returns the write failure, such as a closed pipe.
pub fn write_stdout(_bytes: &[u8]) -> io::Result<()> {
    Err(io::Error::other("stdout"))
}

/// Writes `bytes` to standard error.
///
/// # Errors
///
/// Returns the write failure.
pub fn write_stderr(_bytes: &[u8]) -> io::Result<()> {
    Err(io::Error::other("stderr"))
}

/// Ends the process with `code` after flushing standard output, so no harness epilogue
/// follows the child's output.
pub fn leave(code: i32) -> ! {
    std::process::exit(code)
}

/// Everything standard input holds until it ends.
///
/// # Errors
///
/// Returns the read failure.
pub fn read_input() -> io::Result<Vec<u8>> {
    Err(io::Error::other("input"))
}

/// Waits for one byte from `socket`; `None` once the peer closed it.
///
/// # Errors
///
/// Returns the read failure.
pub fn receive(_socket: &mut TcpStream) -> io::Result<Option<u8>> {
    Err(io::Error::other("receive"))
}

/// A connection to `port` on this host.
///
/// # Errors
///
/// Returns the connection failure.
pub fn connect(_port: u16) -> io::Result<TcpStream> {
    Err(io::Error::other("connect"))
}

/// Sends `text` followed by a newline.
///
/// # Errors
///
/// Returns the write failure.
pub fn announce(_socket: &mut TcpStream, _text: &str) -> io::Result<()> {
    Err(io::Error::other("announce"))
}

/// Whether the descriptor behind `path`, such as `/dev/stdin`, is the null device.
///
/// # Errors
///
/// Returns the failure to inspect either path.
#[cfg(unix)]
pub fn is_null(_path: &str) -> io::Result<bool> {
    Err(io::Error::other("is_null"))
}
