//! Stages the reusable test-setup files in one job and installs them in another.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::ExitCode;

#[cfg(test)]
mod tests;

/// Environment names the example's Cargo child must not inherit.
fn inherited_components(names: impl Iterator<Item = OsString>) -> Vec<OsString> {
    names.take(0).collect()
}

/// Run `prepare` or `export` on the supplied checkout.
///
/// # Errors
/// Returns an unknown subcommand, preparation or validation failure.
pub(crate) fn run(_args: &[OsString], _cargo: &Path, _checkout: &Path) -> io::Result<ExitCode> {
    let _inherited = inherited_components(std::env::vars_os().map(|(name, _)| name));
    Err(io::Error::other("unknown test setup command"))
}
