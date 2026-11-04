//! Prepares and exports the executable a workspace that does not build it needs.
//!
//! A workspace whose tests start Maestro's executable but do not hold its package reaches it
//! through a git dependency at one locked commit. Preparation installs it from that source into
//! the bundle, and the `executable` setup exports the prepared path. A workspace that holds the
//! package builds it with Cargo, so the setup refuses that workspace instead of exporting a
//! second copy of the executable its tests already have.

use std::env::consts::EXE_EXTENSION;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::program::{self, Program, Target};
use super::{at, copy, digest, executable, read_manifest, verify};
use crate::workspace::Workspace;

/// The variable the tests read the prepared executable from.
pub(crate) const VARIABLE: &str = "MAESTRO_EXECUTABLE";
/// The package and binary this setup exports.
const NAME: &str = "maestro";
/// The bundle directory the executable is installed into before it is staged.
const INSTALLED: &str = "executable";

/// Install the executable into the bundle when the workspace reaches it through a dependency,
/// and return the digest it was recorded with.
///
/// # Errors
/// Returns a failed install or copy, or a package the workspace neither holds nor depends on.
pub(crate) fn stage(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
) -> io::Result<Option<String>> {
    if workspace.package(NAME).is_some() {
        return Ok(None);
    }
    let program = Program {
        package: NAME,
        target: Target::Binary(NAME),
    };
    let Some(source) = program::source(cargo, checkout, workspace, NAME)? else {
        return Ok(None);
    };
    let installed = program::install(cargo, checkout, &source, &program, &output.join(INSTALLED))?;
    let staged = output.join(staged_name());
    copy(&installed, &staged)?;
    Ok(Some(digest(&std::fs::read(&staged)?)))
}

/// Export the prepared executable, refusing a workspace that builds it itself.
///
/// # Errors
/// Returns a workspace that holds the package, a run without a prepared bundle, or a stale or
/// missing bundle executable.
pub(crate) fn run(checkout: &Path) -> io::Result<ExitCode> {
    let workspace = Workspace::read(checkout)?;
    if workspace.package(NAME).is_some() {
        return Err(io::Error::other(format!(
            "this workspace builds {NAME} itself, so its tests run Cargo's executable; the \
             executable setup serves a workspace that builds on Maestro without holding it"
        )));
    }
    let bundle = crate::nextest_setup::prepared_bundle(checkout)
        .ok_or_else(|| io::Error::other("the executable setup needs a prepared bundle"))?;
    let path = prepared(&bundle)?;
    let mut entry = OsString::from(format!("{VARIABLE}="));
    entry.push(path);
    crate::nextest_setup::export(&entry)?;
    Ok(ExitCode::SUCCESS)
}

/// Validate the bundle's executable and return its path.
///
/// # Errors
/// Returns a missing or stale bundle executable.
pub(crate) fn prepared(output: &Path) -> io::Result<PathBuf> {
    let output = std::fs::canonicalize(output).map_err(|error| at(output, &error))?;
    let expected = read_manifest(&output)?
        .executable
        .ok_or_else(|| at(&output, "the bundle holds no executable"))?;
    let path = output.join(staged_name());
    verify(&path, &expected)?;
    Ok(path)
}

/// The bundle name of the executable.
fn staged_name() -> PathBuf {
    executable(NAME, EXE_EXTENSION)
}
