//! Native launch observations on a separate socket, leaving both output pipes raw.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};

use maestro_test_process::child::{connect, leave, read_input, write_stderr, write_stdout};

/// The finite observation scenario, parsed completely before connecting or writing.
pub(super) struct Record {
    /// Parent's observation socket.
    port: u16,
    /// Status after the input stream ends.
    status: u8,
}

impl Record {
    /// Read an observation record, rejecting unknown scenarios, fields and invalid numbers.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let fields: Vec<_> = text.lines().collect();
        let ["observe", port, status] = fields.as_slice() else {
            return Err(io::Error::other("expected observe, port and exit status"));
        };
        let port = port.parse::<u16>().map_err(io::Error::other)?;
        if port == 0 {
            return Err(io::Error::other("observation port must not be zero"));
        }
        Ok(Self {
            port,
            status: status.parse().map_err(io::Error::other)?,
        })
    }
}

/// Write one native string with a length prefix, including empty and non-text values.
fn field(output: &mut impl Write, value: &OsStr) -> io::Result<()> {
    let bytes = value.as_encoded_bytes();
    let length = u64::try_from(bytes.len()).map_err(io::Error::other)?;
    output.write_all(&length.to_le_bytes())?;
    output.write_all(bytes)
}

/// Report the actual PID, cwd, environment and untouched operands, then exchange raw bytes.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    if let Some(marker) = std::env::var_os("MTP_EFFECT") {
        std::fs::write(marker, b"started")?;
    }
    let mut socket = connect(record.port)?;
    field(&mut socket, OsStr::new(&std::process::id().to_string()))?;
    field(&mut socket, std::env::current_dir()?.as_os_str())?;
    field(
        &mut socket,
        &std::env::var_os("MTP_VALUE").unwrap_or_default(),
    )?;
    for operand in operands {
        field(&mut socket, operand)?;
    }
    drop(socket);
    write_stderr(b"err\0\xfe!")?;
    write_stdout(&read_input()?)?;
    leave(i32::from(record.status))
}
