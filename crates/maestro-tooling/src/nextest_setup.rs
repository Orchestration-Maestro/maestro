//! The setup commands nextest runs before the tests that need them.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::ExitCode;

/// Run the setup named by the first argument in `checkout`.
///
/// # Errors
/// Returns an unknown setup name, or the failure of the setup.
pub(crate) fn run(_args: &[OsString], _checkout: &Path) -> io::Result<ExitCode> {
    Err(io::Error::other("unknown nextest setup"))
}
