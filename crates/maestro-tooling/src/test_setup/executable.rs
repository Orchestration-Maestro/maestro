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

use serde::{Deserialize, Serialize};

use super::program::{self, Program, Source, Target};
use super::{at, copy, digest, executable, read_manifest, remove_directory, verify};
use crate::workspace::Workspace;

/// The variable the tests read the prepared executable from.
pub(crate) const VARIABLE: &str = "MAESTRO_EXECUTABLE";
/// The package and binary this setup exports.
const NAME: &str = "maestro";
/// The build directory the executable is installed into, beside the other installed programs.
const INSTALLED: &str = "installed";

/// The staged executable and the locked source it was installed from.
#[derive(Serialize, Deserialize)]
pub(super) struct Prepared {
    /// Digest of the staged executable.
    file: String,
    /// The source the workspace pins the package at.
    source: Source,
}

/// Install the executable into `target` when the workspace reaches it through a dependency, and
/// stage it in `output` with the source it was installed from.
///
/// # Errors
/// Returns a failed install or copy, or a locked package without a git source.
pub(super) fn stage(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
    target: &Path,
) -> io::Result<Option<Prepared>> {
    if workspace.member(NAME).is_some() {
        return Ok(None);
    }
    let program = Program {
        package: NAME,
        target: Target::Binary(NAME),
    };
    let Some(source) = program::source(checkout, workspace, NAME)? else {
        return Ok(None);
    };
    let installed = target.join(INSTALLED);
    remove_directory(&installed)?;
    let built = program::install(cargo, checkout, &source, &program, &installed)?;
    let staged = output.join(staged_name());
    copy(&built, &staged)?;
    Ok(Some(Prepared {
        file: digest(&std::fs::read(&staged)?),
        source,
    }))
}

/// Export the prepared executable, refusing a workspace that builds it itself.
///
/// # Errors
/// Returns a workspace that holds the package, a run without a prepared bundle, a bundle prepared
/// from another source, or a missing or stale bundle executable.
pub(crate) fn run(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let workspace = Workspace::read(cargo, checkout)?;
    if workspace.member(NAME).is_some() {
        return Err(io::Error::other(format!(
            "this workspace builds {NAME} itself, so its tests run Cargo's executable; the \
             executable setup serves a workspace that builds on Maestro without holding it"
        )));
    }
    let source = program::locked(checkout, NAME)?.ok_or_else(|| {
        io::Error::other(format!(
            "this workspace depends on no locked {NAME}, so the executable setup serves another"
        ))
    })?;
    let bundle = crate::nextest_setup::prepared_bundle(checkout)
        .ok_or_else(|| io::Error::other("the executable setup needs a prepared bundle"))?;
    let path = prepared(&source, &bundle)?;
    let mut entry = OsString::from(format!("{VARIABLE}="));
    entry.push(path);
    crate::nextest_setup::export(&entry)?;
    Ok(ExitCode::SUCCESS)
}

/// Validate the bundle's executable against the `source` the workspace pins and return its path.
///
/// # Errors
/// Returns a missing bundle executable, one installed from another source, or a stale one.
fn prepared(source: &Source, output: &Path) -> io::Result<PathBuf> {
    let output = maestro_path::fs::canonicalize(output).map_err(|error| at(output, &error))?;
    let staged = read_manifest(&output)?
        .executable
        .ok_or_else(|| at(&output, "the bundle holds no executable"))?;
    if staged.source != *source {
        return Err(at(
            &output,
            "the bundle's executable was installed from another source",
        ));
    }
    let path = output.join(staged_name());
    verify(&path, &staged.file)?;
    Ok(path)
}

/// The bundle name of the executable.
fn staged_name() -> PathBuf {
    executable(NAME, EXE_EXTENSION)
}
