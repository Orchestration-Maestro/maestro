//! Stages the reusable test-setup files in one job and installs them in another.
//!
//! `prepare` runs the component-discovery example in a separate build and stages its component
//! files, the built example, this command and a content manifest. `export` validates that
//! bundle against the checkout, installs the example beside the extracted test executables and
//! writes the component paths for nextest.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::argument;
use super::example_files::{FIXED_VARIABLES, validate_variable};

#[cfg(test)]
mod tests;

/// The setup example that discovers, builds and warms the components.
const EXAMPLE: &str = "prepare_author_component";
/// The bundle file that records what the bundle was prepared from.
const MANIFEST: &str = "components.json";
/// The bundle file that holds this command, so a shard needs no Cargo build to validate.
const EXPORTER: &str = "development";
/// What the components depend on: sources, locked dependencies, the toolchain and the launcher.
const INPUTS: [&str; 8] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "crates",
    "examples",
    "assets",
    ".config/prepare-author-component.sh",
];

/// What `prepare` records and `export` checks.
#[derive(Serialize, Deserialize)]
struct Manifest {
    /// Digest of the files the bundle was prepared from.
    inputs: String,
    /// Digest of each component file, by the variable that exports it.
    components: BTreeMap<String, String>,
    /// Digest of the staged setup example.
    example: String,
}

/// Run `prepare` or `export` on the supplied checkout.
///
/// # Errors
/// Returns an unknown or incomplete subcommand, preparation or validation failure.
pub(crate) fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let output = checkout.join(argument(args, 1, "output")?);
    if output == checkout {
        return Err(io::Error::other("the test setup output needs a path"));
    }
    match args.first().and_then(|name| name.to_str()) {
        Some("prepare") => prepare(
            cargo,
            checkout,
            &output,
            &checkout.join(argument(args, 2, "target")?),
        )?,
        Some("export") => export(
            checkout,
            &output,
            argument(args, 2, "directory")?,
            argument(args, 3, "environment")?,
        )?,
        _ => return Err(io::Error::other("unknown test setup command")),
    }
    Ok(ExitCode::SUCCESS)
}

/// Environment names the example's Cargo child must not inherit.
fn inherited_components(names: impl Iterator<Item = OsString>) -> Vec<OsString> {
    names
        .filter(|name| {
            name.to_str()
                .is_some_and(|name| validate_variable(name).is_ok())
        })
        .collect()
}

/// Run the setup example in a build of its own and stage what it produces in `output`.
fn prepare(cargo: &Path, checkout: &Path, output: &Path, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(output)?;
    remove(&output.join(MANIFEST))?;
    let fingerprint = inputs(checkout)?;
    let environment = target.join("nextest-env");
    remove(&environment)?;
    let mut build = Command::new(cargo);
    build
        .current_dir(checkout)
        .args([
            "run",
            "--quiet",
            "--locked",
            "-p",
            "maestro-extensions-wasm",
        ])
        .args(["--example", EXAMPLE, "--target-dir"])
        .arg(target)
        .env("NEXTEST_ENV", &environment);
    for name in inherited_components(std::env::vars_os().map(|(name, _)| name)) {
        build.env_remove(name);
    }
    let status = build.status()?;
    if !status.success() {
        return Err(io::Error::other(format!("{EXAMPLE} failed: {status}")));
    }
    let mut components = BTreeMap::new();
    for line in std::fs::read_to_string(&environment)?.lines() {
        let (variable, source) = line
            .split_once('=')
            .ok_or_else(|| io::Error::other(format!("invalid component entry: {line}")))?;
        let staged = output.join(format!("{variable}.wasm"));
        copy(Path::new(source), &staged)?;
        components.insert(variable.to_owned(), digest(&std::fs::read(&staged)?));
    }
    if inputs(checkout)? != fingerprint {
        return Err(io::Error::other(format!(
            "inputs changed during preparation: {}",
            output.display()
        )));
    }
    let example = output.join(EXAMPLE);
    copy(&target.join("debug/examples").join(EXAMPLE), &example)?;
    copy(&std::env::current_exe()?, &output.join(EXPORTER))?;
    let manifest = Manifest {
        inputs: fingerprint,
        components,
        example: digest(&std::fs::read(&example)?),
    };
    std::fs::write(output.join(MANIFEST), serde_json::to_vec(&manifest)?)
}

/// Validate the bundle in `output`, install its example in `directory` and write the component
/// paths to `environment`. Nothing is installed or written unless every check passes.
fn export(checkout: &Path, output: &Path, directory: &Path, environment: &Path) -> io::Result<()> {
    let output = std::fs::canonicalize(output).map_err(|error| at(output, &error))?;
    if output.to_string_lossy().contains(['\n', '\r']) {
        return Err(at(&output, "the path must stay on one line"));
    }
    let manifest = output.join(MANIFEST);
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(&manifest).map_err(|error| at(&manifest, &error))?)
            .map_err(|error| at(&manifest, &error))?;
    if manifest.inputs != inputs(checkout)? {
        return Err(at(&output, "stale prepared component inputs"));
    }
    if FIXED_VARIABLES
        .iter()
        .any(|variable| !manifest.components.contains_key(*variable))
    {
        return Err(at(&output, "missing prepared author or context component"));
    }
    let mut lines = Vec::new();
    for (variable, expected) in &manifest.components {
        validate_variable(variable).map_err(|error| at(&output, &error))?;
        let path = output.join(format!("{variable}.wasm"));
        verify(&path, expected)?;
        lines.push(format!("{variable}={}\n", path.display()));
    }
    let staged = output.join(EXAMPLE);
    verify(&staged, &manifest.example)?;
    let installed = directory.join("examples").join(EXAMPLE);
    std::fs::create_dir_all(directory.join("examples"))?;
    copy(&staged, &installed)?;
    std::fs::write(environment, lines.concat())
}

/// Digest every file the components are prepared from, by name and content.
fn inputs(checkout: &Path) -> io::Result<String> {
    let listing = Command::new("git")
        .current_dir(checkout)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
        ])
        .args(INPUTS)
        .output()?;
    if !listing.status.success() {
        return Err(io::Error::other(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&listing.stderr)
        )));
    }
    let names: BTreeSet<&[u8]> = listing
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .collect();
    let mut hash = Sha256::new();
    for name in names {
        hash.update(name);
        hash.update([0]);
        let path: PathBuf = checkout.join(std::str::from_utf8(name).map_err(io::Error::other)?);
        if path.is_file() {
            hash.update(Sha256::digest(std::fs::read(path)?));
        } else {
            hash.update(b"missing");
        }
    }
    Ok(hex(&hash.finalize()))
}

/// Check one bundle file against the digest the manifest recorded for it.
fn verify(path: &Path, expected: &str) -> io::Result<()> {
    let bytes = std::fs::read(path).map_err(|error| at(path, &error))?;
    if digest(&bytes) == expected {
        Ok(())
    } else {
        Err(at(path, "stale prepared file"))
    }
}

/// The lowercase hexadecimal SHA-256 digest of `bytes`.
fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Lowercase hexadecimal text for `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// Copy a file, naming it when the copy fails.
fn copy(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::copy(from, to)
        .map(drop)
        .map_err(|error| at(from, &error))
}

/// Delete a file that may not exist.
fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// An error that names `path`, which the bare operating-system error omits.
fn at(path: &Path, reason: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{}: {reason}", path.display()))
}
