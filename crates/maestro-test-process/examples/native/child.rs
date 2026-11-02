//! Native children that report identity, exchange raw bytes and retain their pipes.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

use maestro_test_process::child::{connect, leave, read_input, write_stderr, write_stdout};
use rustix::process::{getpgrp, getsid};
use serde::Deserialize;

/// Native child scenarios parsed completely before any stream, socket or descendant effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Write the supplied streams, then end with the supplied status.
    Output {
        /// Standard output bytes.
        stdout: Vec<u8>,
        /// Standard error bytes.
        stderr: Vec<u8>,
        /// Status after the writes.
        code: u8,
    },
    /// Report two prefixes, release on one input byte, then relay the remaining input.
    Pending {},
    /// Report the working directory and the untouched operands as one JSON array.
    Arguments {},
    /// Relay standard input, optionally after announcing readiness and with a final tail.
    Relay {
        /// Write the readiness marker before the relayed input.
        ready: bool,
        /// Diagnostic bytes written after the relayed input ends.
        tail: Vec<u8>,
    },
    /// Announce this process on the parent's port, then write after its release byte.
    Announce {
        /// Parent-owned port.
        port: u16,
        /// Standard output written after the release.
        stdout: Vec<u8>,
        /// Standard error written after the release.
        stderr: Vec<u8>,
    },
    /// Announce this process and its group, then await the release byte.
    Grouped {
        /// Parent-owned port.
        port: u16,
    },
    /// Answer the retained challenge, then end like the original child.
    Challenge {
        /// Parent-owned port.
        port: u16,
    },
    /// Write the early output, leave a descendant holding both pipes, and exit.
    Early {
        /// Standard output written before this process exits.
        stdout: Vec<u8>,
        /// Standard error written before this process exits.
        stderr: Vec<u8>,
        /// Status of this process.
        code: u8,
        /// Installed alias whose record writes the late output.
        holder: String,
    },
    /// Report this process identity, optionally leaving a group peer, then await input.
    Identity {
        /// Installed alias whose record holds a group peer on the same session.
        holder: Option<String>,
    },
}

impl Record {
    /// Reject unknown scenarios and extra fields before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("child\n")
            .ok_or_else(|| io::Error::other("missing child family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Run the selected child with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Output {
            stdout,
            stderr,
            code,
        } => {
            write_stdout(stdout)?;
            write_stderr(stderr)?;
            leave(i32::from(*code))
        }
        Record::Pending {} => pending(),
        Record::Arguments {} => arguments(operands),
        Record::Relay { ready, tail } => relay(*ready, tail),
        Record::Announce {
            port,
            stdout,
            stderr,
        } => announce(*port, stdout, stderr),
        Record::Grouped { port } => grouped(*port),
        Record::Challenge { port } => challenge(*port),
        Record::Early {
            stdout,
            stderr,
            code,
            holder,
        } => early(stdout, stderr, *code, holder),
        Record::Identity { holder } => identity(holder.as_deref()),
    }
}

/// This process's group identity, in the width a process id uses.
fn group() -> io::Result<u64> {
    u64::try_from(getpgrp().as_raw_pid()).map_err(io::Error::other)
}

/// This process's session identity, in the width a process id uses.
fn session() -> io::Result<u64> {
    u64::try_from(getsid(None)?.as_raw_pid()).map_err(io::Error::other)
}

/// Report two prefixes, then relay the input that follows the release byte.
fn pending() -> io::Result<()> {
    write_stdout(&[0, 255, 226])?;
    write_stderr(b"error\0")?;
    let mut release = [0; 1];
    io::stdin().read_exact(&mut release)?;
    write_stdout(&[130, 172, release[0]])?;
    write_stderr(b"last")?;
    write_stdout(&read_input()?)
}

/// Report the working directory and the literal operands without interpreting either.
fn arguments(operands: &[OsString]) -> io::Result<()> {
    let operands: Vec<_> = operands
        .iter()
        .map(|value| value.to_string_lossy())
        .collect();
    let report = serde_json::to_vec(&serde_json::json!([std::env::current_dir()?, operands]))?;
    write_stdout(&report)
}

/// Relay standard input to standard output until it ends.
fn relay(ready: bool, tail: &[u8]) -> io::Result<()> {
    if ready {
        write_stdout(b"ready")?;
    }
    write_stdout(&read_input()?)?;
    write_stderr(tail)
}

/// Report this process, then write the final output after the parent's release byte.
fn announce(port: u16, stdout: &[u8], stderr: &[u8]) -> io::Result<()> {
    let mut socket = connect(port)?;
    socket.write_all(&u64::from(std::process::id()).to_be_bytes())?;
    socket.read_exact(&mut [0; 1])?;
    write_stdout(stdout)?;
    write_stderr(stderr)?;
    leave(0)
}

/// Report this process and its group, then await the parent's release byte.
fn grouped(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    socket.write_all(&u64::from(std::process::id()).to_be_bytes())?;
    socket.write_all(&group()?.to_be_bytes())?;
    socket.read_exact(&mut [0; 1])?;
    leave(0)
}

/// Answer the retained challenge with the identity and nonce the parent expects.
fn challenge(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    let id = u64::from(std::process::id());
    socket.write_all(&id.to_be_bytes())?;
    socket.write_all(&session()?.to_be_bytes())?;
    let mut nonce = [0; 16];
    socket.read_exact(&mut nonce)?;
    socket.write_all(&id.to_be_bytes())?;
    socket.write_all(&nonce)?;
    socket.read_exact(&mut [0; 1])?;
    leave(23)
}

/// Leave a descendant holding both inherited pipes, then exit with the early output.
fn early(stdout: &[u8], stderr: &[u8], code: u8, holder: &str) -> io::Result<()> {
    write_stdout(stdout)?;
    write_stderr(stderr)?;
    let descendant = Command::new(holder)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    // The descendant deliberately outlives this process and retains both output pipes.
    drop(descendant);
    leave(i32::from(code))
}

/// Report identity on standard output, optionally leaving a group peer, then await input.
fn identity(holder: Option<&str>) -> io::Result<()> {
    let report = serde_json::json!([u64::from(std::process::id()), group()?, session()?]);
    let mut line = serde_json::to_vec(&report)?;
    line.push(b'\n');
    write_stdout(&line)?;
    if let Some(holder) = holder {
        let descendant = Command::new(holder)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;
        // The descendant outlives this process so a group signal reaches both.
        drop(descendant);
    }
    io::stdin().read_exact(&mut [0; 1])?;
    leave(0)
}
