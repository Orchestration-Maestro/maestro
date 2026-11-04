//! Build, fingerprint and stage the host-native test fixtures.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::inputs::Inputs;
use super::plan::closure_directories;
use super::program::{self, Program, Source, Target};
use super::{SHARED_INPUTS, at, copy, digest, verify};
use crate::workspace::Workspace;

/// The explicit native artifact exported to tests.
pub(crate) const VARIABLE: &str = "MAESTRO_NATIVE_FIXTURE";
/// The instrumented parent that proves the toolchain wrapper keeps coverage counters.
pub(crate) const PROBE_VARIABLE: &str = "MAESTRO_CHILD_TOOLCHAIN_PROBE";
/// The one harness-free example target.
const EXAMPLE: &str = "native_fixture";
/// The package that owns the fixture.
const PACKAGE: &str = "maestro-test-process";
/// The example target that builds a wasm child from an instrumented parent.
const PROBE_EXAMPLE: &str = "child_toolchain_probe";
/// The package that owns the probe.
const PROBE_PACKAGE: &str = "maestro-tooling";
/// The fixture's record in the prepared bundle.
const RECEIPT: &str = "native-fixture.json";

/// The staged bytes of one host executable and what Cargo reported when it built them.
#[derive(Serialize, Deserialize)]
struct Staged {
    /// Cargo's emitted executable path, retained for preparation diagnostics.
    emitted: String,
    /// Content digest of the staged executable.
    file: String,
}

/// The staged host executables and the exact inputs that produced them.
#[derive(Serialize, Deserialize)]
struct Prepared {
    /// Source and lock fingerprint.
    inputs: Inputs,
    /// The native child every fixture test installs.
    fixture: Staged,
    /// The instrumented probe, in a workspace that builds its own tooling.
    probe: Option<Staged>,
    /// Host identity that must match the executing setup.
    host: String,
}

/// The staged artifact's platform-specific name.
fn filename() -> String {
    format!("{EXAMPLE}{}", std::env::consts::EXE_SUFFIX)
}

/// The staged probe's platform-specific name.
fn probe_filename() -> String {
    format!("{PROBE_EXAMPLE}{}", std::env::consts::EXE_SUFFIX)
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

/// Validate source freshness, platform and bytes before returning the staged paths.
///
/// # Errors
/// Returns a stale record, another host's artifact, or bytes that differ from the record.
pub(crate) fn prepared(checkout: &Path, output: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let record = receipt(output)?;
    record.inputs.verify(checkout, output)?;
    if record.host != host() {
        return Err(at(
            output,
            "prepared native fixture belongs to another host",
        ));
    }
    let mut paths = vec![(
        VARIABLE.to_owned(),
        staged(output, filename(), &record.fixture)?,
    )];
    if let Some(probe) = &record.probe {
        paths.push((
            PROBE_VARIABLE.to_owned(),
            staged(output, probe_filename(), probe)?,
        ));
    }
    Ok(paths)
}

/// Reuse current fixtures or build and stage them once through the locked host dev profile.
///
/// The probe is staged only where this workspace builds its own tooling, because only that
/// workspace's tests launch it.
pub(crate) fn prepare(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
    target: &Path,
) -> io::Result<Vec<(String, PathBuf)>> {
    if let Ok(paths) = prepared(checkout, output) {
        return Ok(paths);
    }
    let inputs = inputs(workspace)?;
    let source = program::source(checkout, workspace, PACKAGE)?.ok_or_else(|| {
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
    let probe = if workspace.member(PROBE_PACKAGE).is_some() {
        Some(build_probe(cargo, checkout, target)?)
    } else {
        None
    };
    inputs.verify(checkout, output)?;
    std::fs::create_dir_all(output)?;
    let fixture = stage(output, filename(), &emitted)?;
    let probe = probe
        .map(|emitted| stage(output, probe_filename(), &emitted))
        .transpose()?;
    let record = Prepared {
        inputs,
        fixture,
        probe,
        host: host(),
    };
    std::fs::write(output.join(RECEIPT), serde_json::to_vec(&record)?)?;
    prepared(checkout, output)
}

/// The fixtures' inputs: the crates they build from and the shared workspace files, which name
/// the locked source when the workspace only depends on a package.
///
/// # Errors
/// Returns a failed metadata read, or a failed listing or read of the workspace files.
fn inputs(workspace: &Workspace) -> io::Result<Inputs> {
    let packages = workspace.holds(&[PACKAGE, PROBE_PACKAGE]);
    let mut paths = closure_directories(workspace.metadata(), &packages)?;
    paths.extend(
        SHARED_INPUTS
            .into_iter()
            .chain(["Cargo.lock"])
            .map(str::to_owned),
    );
    Inputs::new(workspace.root(), paths, BTreeSet::new())
}

/// Validate one staged executable and return its absolute path.
fn staged(output: &Path, name: String, record: &Staged) -> io::Result<PathBuf> {
    let path = output.join(name);
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

/// Copy one executable into the bundle and record its bytes.
fn stage(output: &Path, name: String, emitted: &Path) -> io::Result<Staged> {
    let bytes = std::fs::read(emitted)?;
    copy(emitted, &output.join(name))?;
    Ok(Staged {
        emitted: emitted.to_string_lossy().into_owned(),
        file: digest(&bytes),
    })
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
    reported(&output, EXAMPLE)
}

/// Build the probe with the coverage instrumentation its counters come from.
fn build_probe(cargo: &Path, checkout: &Path, target: &Path) -> io::Result<PathBuf> {
    let output = Command::new(cargo)
        .current_dir(checkout)
        .args([
            "rustc",
            "--locked",
            "-p",
            PROBE_PACKAGE,
            "--example",
            PROBE_EXAMPLE,
            "--profile",
            "dev",
            "--message-format=json-render-diagnostics",
            "--target-dir",
        ])
        .arg(target)
        .args(["--", "-C", "instrument-coverage"])
        .output()?;
    reported(&output, PROBE_EXAMPLE)
}

/// Require a successful build and Cargo's reported executable for the named target.
fn reported(output: &std::process::Output, name: &str) -> io::Result<PathBuf> {
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{name} build failed: {}\n{}",
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
        if message["reason"] == "compiler-artifact" && message["target"]["name"] == name {
            executable = message["executable"]
                .as_str()
                .map(PathBuf::from)
                .or(executable);
        }
    }
    executable.ok_or_else(|| io::Error::other(format!("Cargo reported no {name} executable")))
}
