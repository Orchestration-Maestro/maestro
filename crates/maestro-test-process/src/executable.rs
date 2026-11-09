//! Installation of prepared native fixtures or pinned search programs.

use std::io;
use std::path::{Path, PathBuf};
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

/// Install a supplied `rg` or `fd` into a caller-owned directory, independently of PATH.
///
/// Reads `MAESTRO_TEST_<PROGRAM>` and copies its bytes and permissions to the native program
/// name, including `EXE_SUFFIX`. The caller creates the directory and retains its guard.
///
/// # Errors
/// Returns an unsupported program, a missing export or a failed copy naming its source.
///
/// # Examples
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// let bin = maestro_test_directory::Scratch::new()?;
/// let fd = maestro_test_process::install_search_program("fd", bin.path())?;
/// # Ok(())
/// # }
/// ```
pub fn install_search_program(program: &str, directory: &Path) -> io::Result<PathBuf> {
    if !matches!(program, "rg" | "fd") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown search program",
        ));
    }
    let variable = format!("MAESTRO_TEST_{}", program.to_ascii_uppercase());
    let source = std::env::var_os(&variable)
        .ok_or_else(|| io::Error::other(format!("{program}: nextest must supply {variable}")))?;
    let destination = directory.join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(&source, &destination).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("{}: {error}", Path::new(&source).display()),
        )
    })?;
    Ok(destination)
}

/// Install the prepared artifact as `entry` with the `command` family's `record`.
///
/// The record is one JSON object naming a scenario and its fields; the installed program reads
/// it from the sibling `.fixture` path instead of harness arguments.
///
/// # Errors
///
/// Returns a missing preparation variable, a failed installation or a failed record write. The
/// caller creates the entry's parent directory.
pub fn install_command(entry: &Path, record: &str) -> io::Result<()> {
    install_recorded(entry, "command", record)
}

/// Install the prepared artifact as `entry` and write its fixture family and scenario record.
///
/// The record is the two lines `family` and `scenario` beside the alias, which the installed
/// program reads instead of harness arguments.
///
/// # Errors
///
/// Returns a missing preparation variable, a failed installation or a failed record write.
/// The caller creates the entry's parent directory.
pub(crate) fn install_recorded(entry: &Path, family: &str, scenario: &str) -> io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| io::Error::other("run just prepare-tests, then cargo nextest"))?;
    install_executable(Path::new(&artifact), entry)?;
    let mut record = entry.as_os_str().to_owned();
    record.push(".fixture");
    std::fs::write(record, format!("{family}\n{scenario}\n"))
}
