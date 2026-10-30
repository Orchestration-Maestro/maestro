//! Stages the reusable test-setup files in one job and installs them in another.
//!
//! `prepare` runs the component-discovery example in a separate build and stages its component
//! files, the code it compiled, the built example, this command and a content manifest.
//! `install` stages this command alone, for runs without a bundle. The staged command's
//! `author-component` setup validates the bundle against the checkout, installs the example
//! beside the extracted test executables and the compiled code in the user cache, and writes
//! the component paths for nextest.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use cargo_metadata::Metadata;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::argument;
use super::example_files::{FIXED_VARIABLES, validate_variable};
use super::member_graph::{closure, dependency_edges, member_roots, members};
use compiled_code::CompiledCode;
use user_cache::user_cache_home;

mod compiled_code;
#[cfg(test)]
mod tests;
#[path = "../../maestro-test-process/src/cache.rs"]
/// The user cache location the test-process owner names, shared so a shard installs the compiled
/// code where its test children look for it.
mod user_cache;

/// The setup example that discovers, builds and warms the components.
pub(crate) const EXAMPLE: &str = "prepare_author_component";
/// The bundle file that records what the bundle was prepared from.
pub(crate) const MANIFEST: &str = "components.json";
/// The bundle file that holds this command, so a shard needs no Cargo build to validate.
const EXPORTER: &str = "development";
/// The bundle directory that holds the code the setup example compiled.
const COMPILED: &str = "cache";
/// The workspace packages whose code the preparation builds: the setup example and the example
/// components belong to the first, the development tool that runs the preparation to the second.
const BUILT: [&str; 2] = ["maestro-extensions-wasm", "maestro-tooling"];
/// What the components depend on besides the crates: locked dependencies, the toolchain, the
/// example sources and the assets.
const FIXED_INPUTS: [&str; 6] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "examples",
    "assets",
];

/// What `prepare` records and `export` checks.
#[derive(Serialize, Deserialize)]
struct Manifest {
    /// Digest of the files the bundle was prepared from.
    inputs: String,
    /// Repository-relative directories of the workspace crates those files include.
    crates: BTreeSet<String>,
    /// Digest of each component file, by the variable that exports it.
    components: BTreeMap<String, String>,
    /// Digest of the staged setup example.
    example: String,
    /// The code the example compiled, when the host keeps it in a user cache.
    compiled: Option<CompiledCode>,
}

/// Run `prepare` or `install` on the supplied checkout.
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
        Some("install") => {
            std::fs::create_dir_all(&output)?;
            stage_tool(&output)?;
        }
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

/// Build the setup example in a build of its own, run it with a cache of its own and stage what
/// it produces in `output`.
fn prepare(cargo: &Path, checkout: &Path, output: &Path, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(output)?;
    remove(&output.join(MANIFEST))?;
    remove_directory(&output.join(COMPILED))?;
    let crates = built_crates(cargo, checkout)?;
    let fingerprint = inputs(checkout, &crates)?;
    let environment = target.join("nextest-env");
    remove(&environment)?;
    let cache_home = target.join("cache-home");
    remove_directory(&cache_home)?;
    build_example(cargo, checkout, target, "dev")?;
    let example = target.join("debug/examples").join(EXAMPLE);
    run_example(cargo, checkout, &example, &environment, &cache_home)?;
    let mut components = BTreeMap::new();
    for line in std::fs::read_to_string(&environment)?.lines() {
        let (variable, source) = line
            .split_once('=')
            .ok_or_else(|| io::Error::other(format!("invalid component entry: {line}")))?;
        let staged = output.join(format!("{variable}.wasm"));
        copy(Path::new(source), &staged)?;
        components.insert(variable.to_owned(), digest(&std::fs::read(&staged)?));
    }
    if inputs(checkout, &crates)? != fingerprint {
        return Err(io::Error::other(format!(
            "inputs changed during preparation: {}",
            output.display()
        )));
    }
    let compiled = CompiledCode::collect(&cache_home, &example, &output.join(COMPILED))?;
    let staged = output.join(EXAMPLE);
    copy(&example, &staged)?;
    stage_tool(output)?;
    let manifest = Manifest {
        inputs: fingerprint,
        crates,
        components,
        example: digest(&std::fs::read(&staged)?),
        compiled,
    };
    std::fs::write(output.join(MANIFEST), serde_json::to_vec(&manifest)?)
}

/// Copy this command into `output`, where the nextest setup entries run it.
fn stage_tool(output: &Path) -> io::Result<()> {
    copy(&std::env::current_exe()?, &output.join(EXPORTER))
}

/// Build the setup example in `target` with the Cargo `profile`.
pub(crate) fn build_example(
    cargo: &Path,
    checkout: &Path,
    target: &Path,
    profile: &str,
) -> io::Result<()> {
    let status = Command::new(cargo)
        .current_dir(checkout)
        .args([
            "build",
            "--quiet",
            "--locked",
            "-p",
            "maestro-extensions-wasm",
        ])
        .args(["--example", EXAMPLE, "--target-dir"])
        .arg(target)
        .args(["--profile", profile])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{EXAMPLE} failed to build: {status}"
        )))
    }
}

/// Run the built example so it writes its component paths to `environment` and its compiled
/// code under `cache_home`.
fn run_example(
    cargo: &Path,
    checkout: &Path,
    example: &Path,
    environment: &Path,
    cache_home: &Path,
) -> io::Result<()> {
    let mut run = Command::new(example);
    run.current_dir(checkout)
        .env("CARGO", cargo)
        .env("NEXTEST_ENV", environment)
        .env("XDG_CACHE_HOME", cache_home);
    for name in inherited_components(std::env::vars_os().map(|(name, _)| name)) {
        run.env_remove(name);
    }
    let status = run.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{EXAMPLE} failed: {status}")))
    }
}

/// Validate the bundle in `output`, install its example in `directory` and its compiled code in
/// the user cache, and write the component paths to `environment`. Nothing is installed or
/// written unless every check passes.
///
/// # Errors
/// Returns a missing, stale or incomplete bundle, or a failed copy.
pub(crate) fn install_bundle(
    checkout: &Path,
    output: &Path,
    directory: &Path,
    environment: &Path,
) -> io::Result<()> {
    export(
        checkout,
        output,
        directory,
        environment,
        user_cache_home().as_deref(),
    )
}

/// Validate the bundle in `output` and install it, with the user cache named by `user_cache`.
fn export(
    checkout: &Path,
    output: &Path,
    directory: &Path,
    environment: &Path,
    user_cache: Option<&Path>,
) -> io::Result<()> {
    let output = std::fs::canonicalize(output).map_err(|error| at(output, &error))?;
    if output.to_string_lossy().contains(['\n', '\r']) {
        return Err(at(&output, "the path must stay on one line"));
    }
    let manifest = output.join(MANIFEST);
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(&manifest).map_err(|error| at(&manifest, &error))?)
            .map_err(|error| at(&manifest, &error))?;
    if manifest.inputs != inputs(checkout, &manifest.crates)? {
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
    let compiled = output.join(COMPILED);
    if let Some(code) = &manifest.compiled {
        code.verify(&compiled)?;
    }
    let installed = directory.join("examples").join(EXAMPLE);
    std::fs::create_dir_all(directory.join("examples"))?;
    copy(&staged, &installed)?;
    if let (Some(code), Some(user_cache)) = (&manifest.compiled, user_cache) {
        code.install(&compiled, user_cache, &installed)?;
    }
    std::fs::write(environment, lines.concat())
}

/// The directories of the workspace crates the preparation builds and every crate they depend on.
fn built_crates(cargo: &Path, checkout: &Path) -> io::Result<BTreeSet<String>> {
    closure_directories(&crate::locked_metadata(cargo, checkout)?, &BUILT)
}

/// The repository-relative directories and files the components are prepared from.
fn input_names(crates: &BTreeSet<String>) -> impl Iterator<Item = &str> {
    FIXED_INPUTS
        .into_iter()
        .chain(crates.iter().map(String::as_str))
}

/// The directories of the named workspace packages and of the packages they depend on.
fn closure_directories(metadata: &Metadata, built: &[&str]) -> io::Result<BTreeSet<String>> {
    let members = members(metadata);
    let roots = member_roots(&members);
    let start = built
        .iter()
        .map(|name| {
            members
                .iter()
                .position(|package| package.name == *name)
                .ok_or_else(|| io::Error::other(format!("{name} is not a workspace package")))
        })
        .collect::<io::Result<HashSet<_>>>()?;
    Ok(closure(&dependency_edges(&members, &roots), start)
        .into_iter()
        .filter_map(|member| {
            let directory = members[member].manifest_path.parent()?;
            let relative = directory.strip_prefix(&metadata.workspace_root).ok()?;
            Some(
                relative
                    .components()
                    .map(|part| part.as_str())
                    .collect::<Vec<_>>()
                    .join("/"),
            )
        })
        .collect())
}

/// Digest every file the components are prepared from, by name and content.
fn inputs(checkout: &Path, crates: &BTreeSet<String>) -> io::Result<String> {
    let mut hash = Sha256::new();
    for name in listed(checkout, crates)? {
        hash.update(name.as_bytes());
        hash.update([0]);
        let path = checkout.join(&name);
        if path.is_file() {
            hash.update(Sha256::digest(std::fs::read(path)?));
        } else {
            hash.update(b"missing");
        }
    }
    Ok(hex(&hash.finalize()))
}

/// The repository-relative names of the files the components are prepared from, sorted.
fn listed(checkout: &Path, crates: &BTreeSet<String>) -> io::Result<BTreeSet<String>> {
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
        .args(input_names(crates))
        .output()?;
    if !listing.status.success() {
        return Err(io::Error::other(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&listing.stderr)
        )));
    }
    listing
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8(name.to_vec()).map_err(io::Error::other))
        .collect()
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
pub(crate) fn hex(bytes: &[u8]) -> String {
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

/// Delete a directory tree that may not exist.
fn remove_directory(path: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// An error that names `path`, which the bare operating-system error omits.
fn at(path: &Path, reason: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{}: {reason}", path.display()))
}
