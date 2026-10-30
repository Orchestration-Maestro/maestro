//! The setup commands nextest runs before the tests that need them.
//!
//! Each setup exports its variables by appending `NAME=value` lines to the file that
//! `NEXTEST_ENV` names, so the tests it serves inherit them.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write as _};
use std::path::Path;
use std::process::ExitCode;

use maestro_path::normalize_native;

use super::argument;
use crate::test_setup::at;

/// Prepare and launch the component-discovery example.
mod author_component;
/// Prepare the browser build the browser author tests share.
mod browser_graph;
/// Vendor the registry crates the tooling fixtures resolve.
mod fixture_sources;
/// Align the modification times of the executables the example warms code for.
mod normalize;

/// Run the setup named by the first argument in the current directory, where nextest starts
/// its setup scripts: the workspace root.
///
/// # Errors
/// Returns an unknown or incomplete setup name, or the failure of the setup.
pub(crate) fn run(args: &[OsString]) -> io::Result<ExitCode> {
    let checkout = normalize_native(&std::env::current_dir()?);
    match args.first().and_then(|name| name.to_str()) {
        Some("author-component") => author_component::run(&checkout),
        Some("fixture-sources") => fixture_sources::run(&checkout),
        Some("browser-author") => browser_graph::run(&checkout),
        Some("normalize-component-cache") => normalize::run(
            &checkout,
            &checkout.join(argument(args, 1, "directory")?),
            normalize::archive_signalled(),
        ),
        _ => Err(io::Error::other("unknown nextest setup")),
    }
}

/// The Cargo executable the setup was given, else the one on the search path.
fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// The value of an environment variable that is set and not empty.
fn nonempty(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

/// Export one `NAME=value` entry to the tests, when nextest supplied a file for it.
fn export(entry: &OsStr) -> io::Result<()> {
    let Some(file) = std::env::var_os("NEXTEST_ENV") else {
        return Ok(());
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|error| at(Path::new(&file), error))?;
    file.write_all(entry.as_encoded_bytes())?;
    file.write_all(b"\n")
}
