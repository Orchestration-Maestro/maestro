//! Controlled desktop viewer selected by a sibling JSON record.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;

use maestro_test_process::child::{connect, leave};
use serde::Deserialize;

/// The parent's port, parsed before the viewer connects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    /// Loopback port announcing the launch and the release byte.
    port: u16,
}

impl Record {
    /// Require the family and one loopback port before any connection.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("browser_launch\n")
            .ok_or_else(|| io::Error::other("missing browser launch family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// The name this program was launched by, without its directory.
fn program() -> io::Result<String> {
    let program = std::env::args_os()
        .next()
        .ok_or_else(|| io::Error::other("missing viewer identity"))?;
    Path::new(&program)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("the viewer identity names no program"))
}

/// Announce the program's own name and its literal operands on one line.
fn announce(stream: &mut impl Write, operands: &[OsString]) -> io::Result<()> {
    let operands: Vec<_> = operands
        .iter()
        .map(|value| value.to_string_lossy())
        .collect();
    let record = serde_json::to_vec(&(program()?, operands))?;
    stream.write_all(&record)?;
    stream.write_all(b"\n")
}

/// Report the launch, wait for the parent's release, then end with the viewer's status.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    let mut stream = connect(record.port)?;
    announce(&mut stream, operands)?;
    let mut released = [0];
    stream.read_exact(&mut released)?;
    if released != *b"!" {
        return Err(io::Error::other("missing parent release"));
    }
    stream.write_all(b"done")?;
    leave(1)
}
