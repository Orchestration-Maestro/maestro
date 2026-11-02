//! Installation of prepared native fixtures without writable executable handles in the runner.

use std::io;
use std::path::Path;
use std::process::Command;

/// Copy a prepared native fixture to `destination` through its private installation mode.
///
/// The artifact must be the harness-free `native_fixture` example supplied by repository
/// preparation. The caller owns the destination directory and its lifetime. This operation
/// waits until the copy's writer has closed; the runner never opens a writable executable
/// that a concurrently launched child could inherit. Files are copied, not hard-linked, so
/// callers can change their modification times independently.
///
/// # Errors
///
/// Returns a missing or unlaunchable artifact, a failed copy or permission change, or an
/// unsuccessful installation status. The destination's parent directory must already exist.
///
/// # Examples
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
///     .ok_or_else(|| std::io::Error::other("run just prepare-tests first"))?;
/// let directory = maestro_test_directory::Scratch::new()?;
/// maestro_test_process::install_executable(
///     std::path::Path::new(&artifact),
///     &directory.path().join(format!("ssh{}", std::env::consts::EXE_SUFFIX)),
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn install_executable(artifact: &Path, destination: &Path) -> io::Result<()> {
    let status = Command::new(artifact)
        .arg("--maestro-install")
        .arg(destination)
        .status()
        .map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", artifact.display()))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "cannot install {}: {status}",
            destination.display()
        )))
    }
}
