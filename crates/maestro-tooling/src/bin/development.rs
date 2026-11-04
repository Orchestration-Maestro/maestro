//! Native repository development commands.

use std::io;
use std::path::{Path, PathBuf};

use maestro_tooling as tooling;

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match run(&arguments) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Run the development command where this process starts.
fn run(arguments: &[std::ffi::OsString]) -> io::Result<std::process::ExitCode> {
    let checkout = checkout(std::env::current_dir()?);
    tooling::run(arguments, Path::new(env!("CARGO")), &checkout)
}

/// The workspace the command serves: the root at or above the directory it starts in.
///
/// An installed command is started in the workspace it prepares, so its own source tree names
/// nothing. A command that reads no workspace, such as staging this one, is started in a
/// directory of its own and keeps that directory; a command that reads a workspace reports that
/// it found none.
fn checkout(start: PathBuf) -> PathBuf {
    tooling::workspace::root(&start).unwrap_or(start)
}
