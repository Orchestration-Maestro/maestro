//! Per-test admission for callbacks that share a reserved port.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::process::{Command, ExitCode};

/// Run the supplied test while retaining the optional machine-wide file lock.
///
/// # Errors
/// Returns an unknown wrapper, missing test command, lock failure or child launch failure.
pub(crate) fn run(args: &[OsString]) -> io::Result<ExitCode> {
    let (name, args) = args
        .split_first()
        .ok_or_else(|| io::Error::other("missing nextest wrapper"))?;
    if name != "fixed-port" {
        return Err(io::Error::other("unknown nextest wrapper"));
    }
    let (program, args) = args
        .split_first()
        .ok_or_else(|| io::Error::other("missing wrapped test command"))?;
    let _lock = std::env::var_os("MAESTRO_FIXED_PORT_LOCK")
        .map(|path| {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)?;
            file.lock()?;
            Ok::<_, io::Error>(file)
        })
        .transpose()?;
    Ok(crate::exit_code(Command::new(program).args(args).status()?))
}
