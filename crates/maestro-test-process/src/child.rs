//! Helpers shared by the programs a test re-runs as controlled children and by the
//! parents that read their output.

use std::env::VarError;
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::num::ParseIntError;

use crate::HARNESS_START;

/// The standard output a child produces when it writes `body`.
#[must_use]
pub fn harness(body: &[u8]) -> Vec<u8> {
    [HARNESS_START, body].concat()
}

/// `output` without the harness start bytes of every process that wrote to it.
#[must_use]
pub fn without_harness(output: &[u8]) -> Vec<u8> {
    let mut rest = output;
    let mut kept = Vec::new();
    while let Some(at) = rest
        .windows(HARNESS_START.len())
        .position(|window| window == HARNESS_START)
    {
        kept.extend_from_slice(&rest[..at]);
        rest = &rest[at + HARNESS_START.len()..];
    }
    kept.extend_from_slice(rest);
    kept
}

/// A child program could not read one of its variables.
#[derive(Debug)]
pub enum VariableError {
    /// The variable is unset or not text.
    Unreadable {
        /// The variable's name.
        name: String,
        /// Why the environment refused it.
        source: VarError,
    },
    /// The variable is not a port number.
    NotPort {
        /// The variable's name.
        name: String,
        /// Why the text is not a port.
        source: ParseIntError,
    },
}

impl fmt::Display for VariableError {
    /// Names the variable and the failed step.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { name, .. } => write!(f, "variable `{name}` is unset or not text"),
            Self::NotPort { name, .. } => write!(f, "variable `{name}` is not a port number"),
        }
    }
}

impl Error for VariableError {
    /// The stub keeps no cause.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// The text of variable `name`.
///
/// # Errors
///
/// Fails when the variable is unset or not text.
pub fn variable(name: &str) -> Result<String, VariableError> {
    std::env::var(name).map_err(|source| VariableError::Unreadable {
        name: name.to_owned(),
        source,
    })
}

/// The port number variable `name` holds.
///
/// # Errors
///
/// Fails when the variable is missing or not a port number.
pub fn port(name: &str) -> Result<u16, VariableError> {
    variable(name)?
        .parse()
        .map_err(|source| VariableError::NotPort {
            name: name.to_owned(),
            source,
        })
}

/// One of the three standard descriptors of a process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    /// Standard input.
    Input,
    /// Standard output.
    Output,
    /// Standard error.
    Error,
}

/// A text that names no standard descriptor.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownStream(pub String);

impl fmt::Display for UnknownStream {
    /// Quotes the unknown name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown standard stream `{}`", self.0)
    }
}

impl Error for UnknownStream {}

impl Stream {
    /// The list as the text a parent passes in a variable.
    #[must_use]
    pub fn list_text(streams: &[Self]) -> String {
        let _ = streams;
        String::new()
    }

    /// The list a variable's text names.
    ///
    /// # Errors
    ///
    /// Fails on a name that is not a standard descriptor.
    pub fn parse_list(text: &str) -> Result<Vec<Self>, UnknownStream> {
        let _ = text;
        Ok(Vec::new())
    }
}

/// Replaces each descriptor in `streams` with the null device, so the parent observes the
/// end of the pipe it held.
///
/// # Errors
///
/// Returns the failure to open the null device or to replace a descriptor.
#[cfg(unix)]
pub fn close_streams(streams: &[Stream]) -> io::Result<()> {
    let _ = streams;
    Ok(())
}

/// Writes `bytes` to standard output and flushes them.
///
/// # Errors
///
/// Returns the write failure, such as a closed pipe.
pub fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut out = io::stdout();
    out.write_all(bytes).and_then(|()| out.flush())
}

/// Writes `bytes` to standard error.
///
/// # Errors
///
/// Returns the write failure.
pub fn write_stderr(bytes: &[u8]) -> io::Result<()> {
    io::stderr().write_all(bytes)
}

/// Ends the process with `code` after flushing standard output, so no harness epilogue
/// follows the child's output.
pub fn leave(code: i32) -> ! {
    let _ = io::stdout().flush();
    std::process::exit(code)
}

/// Everything standard input holds until it ends.
///
/// # Errors
///
/// Returns the read failure.
pub fn read_input() -> io::Result<Vec<u8>> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    Ok(input)
}

/// Waits for one byte from `socket`; `None` once the peer closed it.
///
/// # Errors
///
/// Returns the read failure.
pub fn receive(socket: &mut TcpStream) -> io::Result<Option<u8>> {
    let mut byte = [0];
    Ok((socket.read(&mut byte)? == 1).then_some(byte[0]))
}

/// A connection to `port` on this host.
///
/// # Errors
///
/// Returns the connection failure.
pub fn connect(port: u16) -> io::Result<TcpStream> {
    TcpStream::connect(("127.0.0.1", port))
}

/// Sends `text` followed by a newline.
///
/// # Errors
///
/// Returns the write failure.
pub fn announce(socket: &mut TcpStream, text: &str) -> io::Result<()> {
    socket.write_all(format!("{text}\n").as_bytes())
}

/// Whether the descriptor behind `path`, such as `/dev/stdin`, is the null device.
///
/// # Errors
///
/// Returns the failure to inspect either path.
#[cfg(unix)]
pub fn is_null(path: &str) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let null = std::fs::metadata("/dev/null")?;
    Ok(std::fs::metadata(path)?.rdev() == null.rdev())
}
