//! The GitHub command line of the share command: authentication and gist creation frames.

use std::ffi::OsString;
use std::io::{self, Write};
use std::net::TcpStream;
use std::path::Path;

use maestro_test_process::child::{connect, leave, receive, write_stderr, write_stdout};
use rustix::process::{Signal, getpid, kill_process};
use serde::Deserialize;

/// The only signal the authentication step sends itself.
#[derive(Deserialize)]
pub(crate) enum ExitSignal {
    /// Terminate with the native signal rather than an exit status.
    #[serde(rename = "SIGTERM")]
    Terminate,
}

/// The answer to `auth status`.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Auth {
    /// Status when no signal is selected.
    exit: u8,
    /// Remove this program before answering.
    remove_self: bool,
    /// Native termination instead of a status.
    signal: Option<ExitSignal>,
}

/// One step of gist creation.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Frame {
    /// Write these bytes to standard output.
    Stdout(Vec<u8>),
    /// Write these bytes to standard error.
    Stderr(Vec<u8>),
    /// Send these bytes on the control connection.
    Send(Vec<u8>),
    /// Wait until the peer closes the control connection.
    Hold,
    /// Ask for one release on the control connection and wait for the test to grant it.
    Release,
    /// Set the permission bits of the directory holding the exported document.
    Chmod(String),
    /// Copy the exported document into the state directory under this name.
    Copy(String),
}

/// The answer to `gist create`.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Gist {
    /// Loopback port of the control connection.
    port: Option<u16>,
    /// Ordered steps before the status.
    frames: Vec<Frame>,
    /// Status after the last frame.
    exit: u8,
}

/// The complete behavior of one installed program.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Gh {
    /// Directory receiving the operand log and the copied document.
    state: String,
    /// The answer to `auth status`.
    #[serde(default)]
    auth: Auth,
    /// The answer to `gist create`.
    #[serde(default)]
    gist: Gist,
}

/// Log the operands, then answer the authentication or gist request.
pub(super) fn run(gh: &Gh, operands: &[OsString]) -> io::Result<()> {
    let argv: Vec<_> = operands
        .iter()
        .map(|operand| operand.to_string_lossy())
        .collect();
    let mut line = serde_json::to_vec(&argv).map_err(io::Error::other)?;
    line.push(b'\n');
    std::fs::File::options()
        .append(true)
        .create(true)
        .open(Path::new(&gh.state).join("argv.jsonl"))?
        .write_all(&line)?;
    if operands.starts_with(&["auth".into(), "status".into()]) {
        return authenticate(&gh.auth);
    }
    create(gh, operands)
}

/// Answer `auth status` with the recorded outcome.
fn authenticate(auth: &Auth) -> io::Result<()> {
    if auth.remove_self {
        std::fs::remove_file(std::env::current_exe()?)?;
    }
    if let Some(ExitSignal::Terminate) = auth.signal {
        kill_process(getpid(), Signal::TERM)?;
    }
    leave(i32::from(auth.exit))
}

/// Play the gist frames in order against the last operand, the exported document.
fn create(gh: &Gh, operands: &[OsString]) -> io::Result<()> {
    let mut control = gh.gist.port.map(connect).transpose()?;
    for frame in &gh.gist.frames {
        match frame {
            Frame::Stdout(bytes) => write_stdout(bytes)?,
            Frame::Stderr(bytes) => write_stderr(bytes)?,
            Frame::Send(bytes) => channel(&mut control)?.write_all(bytes)?,
            Frame::Hold => {
                receive(channel(&mut control)?)?;
            }
            Frame::Release => {
                let control = channel(&mut control)?;
                control.write_all(b"R")?;
                receive(control)?;
            }
            Frame::Chmod(mode) => {
                let mode = u32::from_str_radix(mode, 8).map_err(io::Error::other)?;
                let document = document(operands)?;
                let directory = Path::new(document)
                    .parent()
                    .ok_or_else(|| io::Error::other("the document has no directory"))?;
                std::fs::set_permissions(directory, mode_from(mode))?;
            }
            Frame::Copy(name) => {
                std::fs::copy(document(operands)?, Path::new(&gh.state).join(name))?;
            }
        }
    }
    leave(i32::from(gh.gist.exit))
}

/// The control connection, which frames that use it require.
fn channel(control: &mut Option<TcpStream>) -> io::Result<&mut TcpStream> {
    control
        .as_mut()
        .ok_or_else(|| io::Error::other("the frame needs a control port"))
}

/// The exported document: the last operand.
fn document(operands: &[OsString]) -> io::Result<&OsString> {
    operands
        .last()
        .ok_or_else(|| io::Error::other("no document operand"))
}

/// Permission bits from a numeric mode.
fn mode_from(mode: u32) -> std::fs::Permissions {
    std::os::unix::fs::PermissionsExt::from_mode(mode)
}
