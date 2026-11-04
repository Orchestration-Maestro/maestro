//! Build, fingerprint and stage the single host-native test fixture.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::inputs::Inputs;
use super::plan::{closure_directories, held};
use super::program::{self, Program, Source, Target};
use super::{SHARED_INPUTS, at, copy, digest, verify};
use crate::workspace::Workspace;

/// The explicit native artifact exported to tests.
pub(crate) const VARIABLE: &str = "MAESTRO_NATIVE_FIXTURE";
/// The one harness-free example target.
const EXAMPLE: &str = "native_fixture";
/// The package that owns the fixture.
const PACKAGE: &str = "maestro-test-process";
/// The fixture's record in the prepared bundle.
const RECEIPT: &str = "native-fixture.json";

/// A staged host executable and the exact inputs that produced it.
#[derive(Serialize, Deserialize)]
struct Prepared {
    /// Source and lock fingerprint.
    inputs: Inputs,
    /// Cargo's emitted executable path, retained for preparation diagnostics.
    emitted: String,
    /// Content digest of the staged executable.
    file: String,
    /// Host identity that must match the executing setup.
    host: String,
}

/// The staged artifact's platform-specific name.
fn filename() -> String {
    format!("{EXAMPLE}{}", std::env::consts::EXE_SUFFIX)
}

/// The host platform this executable can run on.
fn host() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

/// Read the fixture record from an explicit bundle directory.
fn receipt(output: &Path) -> io::Result<Prepared> {
    let path = output.join(RECEIPT);
    serde_json::from_slice(&std::fs::read(&path).map_err(|error| at(&path, error))?)
        .map_err(|error| at(&path, error))
}

/// Validate source freshness, platform and bytes before returning an absolute artifact path.
pub(crate) fn prepared(checkout: &Path, output: &Path) -> io::Result<PathBuf> {
    let record = receipt(output)?;
    record.inputs.verify(checkout, output)?;
    if record.host != host() {
        return Err(at(
            output,
            "prepared native fixture belongs to another host",
        ));
    }
    let path = output.join(filename());
    verify(&path, &record.file)?;
    let path = std::fs::canonicalize(&path)?;
    if path
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .any(|byte| matches!(byte, b'\r' | b'\n'))
    {
        return Err(at(&path, "the path must stay on one line"));
    }
    Ok(path)
}

/// Reuse a current fixture or build and stage it once through the locked host dev profile.
pub(crate) fn prepare(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
    target: &Path,
) -> io::Result<PathBuf> {
    if let Ok(path) = prepared(checkout, output) {
        return Ok(path);
    }
    let inputs = inputs(cargo, checkout)?;
    let source = program::source(cargo, checkout, workspace, PACKAGE)?.ok_or_else(|| {
        io::Error::other(format!(
            "{PACKAGE} is neither a workspace crate nor a locked dependency"
        ))
    })?;
    let emitted = match source {
        Source::Member => build(cargo, checkout, target)?,
        source @ Source::Locked { .. } => {
            let program = Program {
                package: PACKAGE,
                target: Target::Example(EXAMPLE),
            };
            program::install(
                cargo,
                checkout,
                &source,
                &program,
                &target.join("installed"),
            )?
        }
    };
    let bytes = std::fs::read(&emitted)?;
    inputs.verify(checkout, output)?;
    std::fs::create_dir_all(output)?;
    copy(&emitted, &output.join(filename()))?;
    let record = Prepared {
        inputs,
        emitted: emitted.to_string_lossy().into_owned(),
        file: digest(&bytes),
        host: host(),
    };
    std::fs::write(output.join(RECEIPT), serde_json::to_vec(&record)?)?;
    prepared(checkout, output)
}

/// The fixture's inputs: the crates it builds from and the shared workspace files, which name
/// the locked source when the workspace only depends on the package.
///
/// # Errors
/// Returns a failed metadata read, or a failed listing or read of the workspace files.
fn inputs(cargo: &Path, checkout: &Path) -> io::Result<Inputs> {
    let metadata = crate::locked_metadata(cargo, checkout)?;
    let mut paths = closure_directories(&metadata, &held(&metadata, &[PACKAGE]))?;
    paths.extend(
        SHARED_INPUTS
            .into_iter()
            .chain(["Cargo.lock"])
            .map(str::to_owned),
    );
    Inputs::new(checkout, paths, BTreeSet::new())
}

/// Build the one target and accept only Cargo's reported executable, never a directory scan.
fn build(cargo: &Path, checkout: &Path, target: &Path) -> io::Result<PathBuf> {
    let output = Command::new(cargo)
        .current_dir(checkout)
        .args([
            "build",
            "--locked",
            "-p",
            PACKAGE,
            "--example",
            EXAMPLE,
            "--profile",
            "dev",
            "--message-format=json-render-diagnostics",
            "--target-dir",
        ])
        .arg(target)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "native fixture build failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let mut executable = None;
    for line in output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let message: serde_json::Value = serde_json::from_slice(line).map_err(io::Error::other)?;
        if message["reason"] == "compiler-artifact" && message["target"]["name"] == EXAMPLE {
            executable = message["executable"]
                .as_str()
                .map(PathBuf::from)
                .or(executable);
        }
    }
    executable.ok_or_else(|| io::Error::other("Cargo reported no native fixture executable"))
}
