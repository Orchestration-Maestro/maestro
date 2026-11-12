//! Build, fingerprint and stage the host-native test fixtures.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::inputs::Inputs;
use super::plan::closure_directories;
use super::program::{self, Program, Source, Target};
use super::{SHARED_INPUTS, at, copy, digest, verify};
use crate::toolchain;
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
/// The child that runs one startup conversion, started without a test harness.
pub(crate) const CONVERSION_VARIABLE: &str = "MAESTRO_STARTUP_CONVERSION";
/// The example target that runs one startup conversion.
const CONVERSION_EXAMPLE: &str = "startup_conversion";
/// The package that owns the conversion child.
const CONVERSION_PACKAGE: &str = "maestro-app";
/// The fixture's record in the prepared bundle.
const RECEIPT: &str = "native-fixture.json";
/// The conversion child's record, kept apart because its inputs are the application's closure.
const CONVERSION_RECEIPT: &str = "startup-conversion.json";

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
    /// Whether the workspace builds the application, so a conversion child is staged too.
    converts: bool,
    /// Host identity that must match the executing setup.
    host: String,
}

/// The staged startup conversion child and the exact inputs that produced it.
#[derive(Serialize, Deserialize)]
struct Conversion {
    /// Source and lock fingerprint of the application's closure.
    inputs: Inputs,
    /// The child that runs one startup conversion.
    child: Staged,
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

/// The staged conversion child's platform-specific name.
fn conversion_filename() -> String {
    format!("{CONVERSION_EXAMPLE}{}", std::env::consts::EXE_SUFFIX)
}

/// The host platform this executable can run on.
fn host() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

/// Read the record named `name` from an explicit bundle directory.
fn receipt<T: serde::de::DeserializeOwned>(output: &Path, name: &str) -> io::Result<T> {
    let path = output.join(name);
    serde_json::from_slice(&std::fs::read(&path).map_err(|error| at(&path, error))?)
        .map_err(|error| at(&path, error))
}

/// Require the host of the record file `name` to be this one.
fn check_host(output: &Path, name: &str, host_name: &str) -> io::Result<()> {
    if host_name == host() {
        Ok(())
    } else {
        Err(at(
            &output.join(name),
            "prepared native fixture belongs to another host",
        ))
    }
}

/// Whether the workspace builds the application, so a conversion child is staged too.
fn converts(workspace: &Workspace) -> bool {
    workspace.member(CONVERSION_PACKAGE).is_some()
}

/// Validate source freshness, platform and bytes before returning the staged paths.
///
/// # Errors
/// Returns a stale record, another host's artifact, or bytes that differ from the record.
pub(crate) fn prepared(checkout: &Path, output: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let record: Prepared = receipt(output, RECEIPT)?;
    let mut paths = fixtures(checkout, output, &record)?;
    if record.converts {
        paths.push(conversion_prepared(checkout, output)?);
    }
    Ok(paths)
}

/// Validate the native fixture and the probe, which share one record.
fn fixtures(
    checkout: &Path,
    output: &Path,
    record: &Prepared,
) -> io::Result<Vec<(String, PathBuf)>> {
    record.inputs.verify(checkout, output)?;
    check_host(output, RECEIPT, &record.host)?;
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

/// Validate the conversion child against its own record.
fn conversion_prepared(checkout: &Path, output: &Path) -> io::Result<(String, PathBuf)> {
    let record: Conversion = receipt(output, CONVERSION_RECEIPT)?;
    record.inputs.verify(checkout, output)?;
    check_host(output, CONVERSION_RECEIPT, &record.host)?;
    Ok((
        CONVERSION_VARIABLE.to_owned(),
        staged(output, conversion_filename(), &record.child)?,
    ))
}

/// Reuse current fixtures or build and stage them once through the locked host dev profile.
///
/// The probe is staged only where this workspace builds its own tooling, and the conversion
/// child only where it builds the application, because only those workspaces' tests launch them.
/// The child has its own record, so an edit to the application rebuilds it alone and an edit
/// to the fixture's crates leaves it staged.
pub(crate) fn prepare(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
    target: &Path,
) -> io::Result<Vec<(String, PathBuf)>> {
    let conversion = converts(workspace)
        .then(|| prepare_conversion(cargo, checkout, workspace, output, target))
        .transpose()?;
    let mut paths = receipt(output, RECEIPT)
        .and_then(|record| fixtures(checkout, output, &record))
        .or_else(|_| prepare_fixtures(cargo, checkout, workspace, output, target))?;
    paths.extend(conversion);
    Ok(paths)
}

/// Build and stage the conversion child unless its record is current.
fn prepare_conversion(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
    target: &Path,
) -> io::Result<(String, PathBuf)> {
    if let Ok(paths) = conversion_prepared(checkout, output) {
        return Ok(paths);
    }
    let inputs = inputs(workspace, &[CONVERSION_PACKAGE])?;
    let emitted = build(
        cargo,
        checkout,
        target,
        CONVERSION_PACKAGE,
        CONVERSION_EXAMPLE,
    )?;
    inputs.verify(checkout, output)?;
    std::fs::create_dir_all(output)?;
    let child = stage(output, conversion_filename(), &emitted)?;
    let record = Conversion {
        inputs,
        child,
        host: host(),
    };
    std::fs::write(
        output.join(CONVERSION_RECEIPT),
        serde_json::to_vec(&record)?,
    )?;
    conversion_prepared(checkout, output)
}

/// Build and stage the native fixture and the probe, and write their record.
fn prepare_fixtures(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
    target: &Path,
) -> io::Result<Vec<(String, PathBuf)>> {
    let inputs = inputs(workspace, &[PACKAGE, PROBE_PACKAGE])?;
    let source = program::source(checkout, workspace, PACKAGE)?.ok_or_else(|| {
        io::Error::other(format!(
            "{PACKAGE} is neither a workspace crate nor a locked dependency"
        ))
    })?;
    let emitted = match source {
        Source::Member => build(cargo, checkout, target, PACKAGE, EXAMPLE)?,
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
        converts: converts(workspace),
        host: host(),
    };
    std::fs::write(output.join(RECEIPT), serde_json::to_vec(&record)?)?;
    fixtures(checkout, output, &record)
}

/// The inputs of the named packages: the crates they build from and the shared workspace files,
/// which name the locked source when the workspace only depends on a package.
///
/// # Errors
/// Returns a failed metadata read, or a failed listing or read of the workspace files.
fn inputs(workspace: &Workspace, names: &[&str]) -> io::Result<Inputs> {
    let packages = workspace.holds(names);
    let mut paths = closure_directories(workspace.metadata(), &packages)?;
    paths.extend(
        SHARED_INPUTS
            .into_iter()
            .chain(["Cargo.lock"])
            .map(str::to_owned),
    );
    Inputs::new(workspace.root(), paths, BTreeSet::new(), BTreeSet::new())
}

/// Validate one staged executable and return its absolute path.
fn staged(output: &Path, name: String, record: &Staged) -> io::Result<PathBuf> {
    let path = output.join(name);
    verify(&path, &record.file)?;
    let path = maestro_realpath::canonicalize(&path)?;
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
fn build(
    cargo: &Path,
    checkout: &Path,
    target: &Path,
    package: &str,
    example: &str,
) -> io::Result<PathBuf> {
    let output = toolchain::command(cargo, checkout)
        .args([
            "build",
            "--locked",
            "-p",
            package,
            "--example",
            example,
            "--profile",
            "dev",
            "--message-format=json-render-diagnostics",
            "--target-dir",
        ])
        .arg(target)
        .output()?;
    reported(&output, example)
}

/// Build the probe with the coverage instrumentation its counters come from.
fn build_probe(cargo: &Path, checkout: &Path, target: &Path) -> io::Result<PathBuf> {
    let output = toolchain::command(cargo, checkout)
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
