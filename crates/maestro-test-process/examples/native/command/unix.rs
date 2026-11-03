//! The `command` scenarios that need a Unix socket, signal pipe or process replacement.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;

use maestro_test_process::child::{leave, read_input, write_stderr, write_stdout};
use serde::Deserialize;
use signal_hook::consts::{SIGHUP, SIGTERM};

use super::{Append, append_to, remain};

/// A process that records readiness, blocks on a permit and then finishes when released.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Hold {
    /// Marker written immediately before blocking, when a reader waits for it.
    ready: Option<String>,
    /// FIFO read after the marker, which returns when the test writes a byte to it.
    permit: String,
    /// FIFO read before the permit, whose writer may have opened it first.
    late: Option<String>,
    /// Bytes written to standard output after the gate.
    stdout: Vec<u8>,
    /// Bytes written to standard error after the gate.
    stderr: Vec<u8>,
    /// Append performed after the gate.
    append: Option<Append>,
    /// Append performed when this process receives SIGTERM; it then exits with status 0.
    terminate: Option<Append>,
}

/// The single socket path a coordination scenario needs.
fn socket_path(operands: &[OsString]) -> io::Result<&OsString> {
    match operands {
        [path] => Ok(path),
        _ => Err(io::Error::other("expected one socket path")),
    }
}

/// Queue the hangup before readiness, so one that arrives early is not lost, then tell the
/// socket with `h` and run on.
pub(super) fn hold_hangup(operands: &[OsString]) -> io::Result<()> {
    let path = socket_path(operands)?;
    let mut reader = trap(SIGHUP)?;
    write_stdout(b"\nready\n")?;
    reader.read_exact(&mut [0])?;
    UnixStream::connect(path)?.write_all(b"h")?;
    remain()
}

/// Announce with `w`, then wait for the release byte without polling.
pub(super) fn await_release(operands: &[OsString]) -> io::Result<()> {
    let mut socket = UnixStream::connect(socket_path(operands)?)?;
    socket.write_all(b"w")?;
    socket.read_exact(&mut [0])?;
    leave(0)
}

/// Print the operands, the invoked name, one input line and the working directory.
pub(super) fn stdio(operands: &[OsString]) -> io::Result<()> {
    let mut operands_text = String::new();
    for operand in operands {
        operands_text.push('<');
        operands_text.push_str(&operand.to_string_lossy());
        operands_text.push('>');
    }
    let invoked = std::env::args_os().next().unwrap_or_default();
    let name = std::path::Path::new(&invoked)
        .file_name()
        .unwrap_or(invoked.as_os_str());
    let name = name.to_string_lossy();
    write_stdout(format!("{operands_text}out-{name}\n").as_bytes())?;
    write_stderr(format!("err-{name}\n").as_bytes())?;
    let line = read_line()?;
    write_stdout(format!("stdin:{}\n", String::from_utf8_lossy(&line)).as_bytes())?;
    write_stdout(format!("{}\n", std::env::current_dir()?.to_string_lossy()).as_bytes())
}

/// Append the marker, then replace this process with the recorded program.
pub(super) fn forward(marker: &Append, program: &str, operands: &[OsString]) -> io::Result<()> {
    append_to(marker)?;
    Err(std::process::Command::new(program).args(operands).exec())
}

impl Hold {
    /// Drain the shared input, announce readiness, wait for the release and report.
    pub(super) fn run(&self) -> io::Result<()> {
        let termination = self.terminate.as_ref().map(|_| trap(SIGTERM)).transpose()?;
        drop(read_input()?);
        if let Some(ready) = &self.ready {
            std::fs::write(ready, b"ready")?;
        }
        if let Some(late) = &self.late {
            wait_for_release(late)?;
        }
        wait_for_release(&self.permit)?;
        write_stdout(&self.stdout)?;
        write_stderr(&self.stderr)?;
        if let Some(append) = &self.append {
            append_to(append)?;
        }
        if let (Some(mut signal), Some(terminate)) = (termination, &self.terminate) {
            // The handler writes into the pipe, so this wait blocks until SIGTERM arrives.
            signal.read_exact(&mut [0; 1])?;
            append_to(terminate)?;
        }
        leave(0)
    }
}

/// A read end that becomes readable when this process's handler for `signal` writes one byte.
fn trap(signal: i32) -> io::Result<UnixStream> {
    let (reader, writer) = UnixStream::pair()?;
    signal_hook::low_level::pipe::register(signal, writer).map_err(io::Error::other)?;
    Ok(reader)
}

/// Blocks until the test writes one byte, which is the only release a gate waits for.
fn wait_for_release(path: &str) -> io::Result<()> {
    let mut release = std::fs::File::open(path)?;
    release.read_exact(&mut [0; 1])
}

/// Echo the first operand on both streams, then one line of standard input.
pub(super) fn labelled(operands: &[OsString]) -> io::Result<()> {
    let Some(label) = operands.first() else {
        return Err(io::Error::other("missing labelled operand"));
    };
    let label = label.to_string_lossy();
    write_stdout(format!("{label}\n").as_bytes())?;
    write_stderr(format!("{label}-err\n").as_bytes())?;
    let line = read_line()?;
    write_stdout(format!("stdin:{}\n", String::from_utf8_lossy(&line)).as_bytes())
}

/// One line of standard input, read without consuming bytes past its newline.
fn read_line() -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    let mut byte = [0; 1];
    loop {
        match rustix::io::read(rustix::stdio::stdin(), &mut byte) {
            // An ended or unreadable input reports the value read so far, as a shell's
            // failed `read` does.
            Ok(0) | Err(_) => return Ok(line),
            Ok(_) if byte[0] == b'\n' => return Ok(line),
            Ok(_) => line.push(byte[0]),
        }
    }
}
