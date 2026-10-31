//! Stages the reusable test-setup files in one job and installs them in another.
//!
//! `prepare` runs the component-discovery example in a separate build and stages its component
//! files, the code it compiled, the built example, the browser graph the browser author tests
//! share, this command and a content manifest. The manifest records the inputs of each artifact
//! on its own, so a change reaches only the artifacts that read it. The first earlier bundle in the
//! path list `TEST_PREPARE_BASE` that fits supplies every component whose inputs are unchanged, and
//! its compiled code, so only the rest are built.
//! `install` stages this command alone, for runs without a bundle. The staged command's
//! `author-component` setup validates the components and the example against the checkout,
//! installs the example beside the extracted test executables and the compiled code in the
//! user cache, and writes the component paths for nextest. The `browser-author` setup validates
//! and installs the browser graph.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::argument;
use super::example_files::{FIXED_VARIABLES, validate_variable};
use super::nextest_setup::browser_graph;
use base::{Base, Supplied};
use compiled_code::CompiledCode;
use inputs::Inputs;
use plan::{Artifacts, component_inputs};
use user_cache::user_cache_home;

mod base;
mod compiled_code;
mod inputs;
mod plan;
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
/// The environment variable that names an earlier bundle to reuse.
const BASE: &str = "TEST_PREPARE_BASE";
/// The package whose examples are the components.
const COMPONENT_PACKAGE: &str = "maestro-extensions-wasm";
/// The packages the setup example and the development tool that runs it are built from.
const TOOL_PACKAGES: [&str; 2] = [COMPONENT_PACKAGE, "maestro-tooling"];
/// The packages the browser graph is built from.
const BROWSER_PACKAGES: [&str; 2] = ["maestro-web", "maestro-tooling"];
/// The directory of the package's examples, which each component hashes for itself.
const COMPONENT_EXAMPLES: &str = "crates/maestro-extensions-wasm/examples";
/// What every artifact depends on besides the crates: the workspace manifest, the toolchain and
/// the Cargo configuration.
const SHARED_INPUTS: [&str; 3] = ["Cargo.toml", "rust-toolchain.toml", ".cargo"];
/// What the tool and the browser graph read besides the crates and the shared inputs: the locked
/// dependencies and the assets.
const LOCKED_INPUTS: [&str; 2] = ["Cargo.lock", "assets"];
/// What the setup example reads besides those: the example sources.
const EXAMPLE_INPUTS: [&str; 1] = ["examples"];

/// What `prepare` records and `export` checks.
#[derive(Serialize, Deserialize)]
struct Manifest {
    /// The inputs every component shares: the crates and locked packages the components build
    /// on, the workspace manifest and the toolchain.
    shared: Inputs,
    /// The inputs of the setup example and of the development tool that runs it.
    tool: Inputs,
    /// The inputs of the browser graph.
    browser: Inputs,
    /// Each component file, by the variable that exports it.
    components: BTreeMap<String, Component>,
    /// Digest of the staged setup example.
    example: String,
    /// The code the example compiled, when the host keeps it in a user cache.
    compiled: Option<CompiledCode>,
}

/// One staged component.
#[derive(Serialize, Deserialize)]
struct Component {
    /// Digest of the component file.
    file: String,
    /// The example files it was built from, beside the shared inputs.
    inputs: Inputs,
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
/// it produces in `output`. Components and compiled code come from the first bundle in the path
/// list `TEST_PREPARE_BASE` names that was prepared from the same shared inputs.
fn prepare(cargo: &Path, checkout: &Path, output: &Path, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(output)?;
    let artifacts = Artifacts::read(cargo, checkout)?;
    let base = std::env::var_os(BASE).and_then(|list| {
        std::env::split_paths(&list)
            .filter(|name| !name.as_os_str().is_empty())
            .find_map(|name| Base::read(&checkout.join(name), &artifacts.shared))
    });
    remove(&output.join(MANIFEST))?;
    remove(&environment_file(target))?;
    remove_directory(&cache_home(target))?;
    build_example(cargo, checkout, target, "dev")?;
    let example = target.join("debug/examples").join(EXAMPLE);
    let (supplied, seeded) = match &base {
        Some(base) => (
            base.supplied(checkout)?,
            base.seed(&cache_home(target), &example)?,
        ),
        None => (Vec::new(), None),
    };
    run_example(cargo, checkout, &example, target, &supplied)?;
    let components = stage_components(checkout, output, target, &artifacts.shared, &supplied)?;
    stage_browser_graph(cargo, checkout, output)?;
    let mut unchanged = artifacts.is_current(checkout)?;
    for component in components.values() {
        unchanged &= component.inputs.is_current(checkout)?;
    }
    if !unchanged {
        return Err(io::Error::other(format!(
            "inputs changed during preparation: {}",
            output.display()
        )));
    }
    remove_directory(&output.join(COMPILED))?;
    let compiled = CompiledCode::collect(
        &cache_home(target),
        &example,
        &output.join(COMPILED),
        seeded.as_deref(),
    )?;
    let staged = output.join(EXAMPLE);
    copy(&example, &staged)?;
    stage_tool(output)?;
    let manifest = Manifest {
        shared: artifacts.shared,
        tool: artifacts.tool,
        browser: artifacts.browser,
        components,
        example: digest(&std::fs::read(&staged)?),
        compiled,
    };
    std::fs::write(output.join(MANIFEST), serde_json::to_vec(&manifest)?)?;
    let reused = supplied.len();
    let built = manifest.components.len() - reused;
    writeln!(io::stdout(), "components: {reused} reused, {built} built")
}

/// The file the setup example writes its component paths to.
fn environment_file(target: &Path) -> PathBuf {
    target.join("nextest-env")
}

/// The user cache the setup example compiles into.
fn cache_home(target: &Path) -> PathBuf {
    target.join("cache-home")
}

/// Stage each component the setup example exported in `output`, with the example files it was
/// built from: those the example reported in Cargo's dependency file, or those the base recorded
/// for a component it supplied.
fn stage_components(
    checkout: &Path,
    output: &Path,
    target: &Path,
    shared: &Inputs,
    supplied: &[Supplied],
) -> io::Result<BTreeMap<String, Component>> {
    let shared = shared.files(checkout)?;
    let mut components = BTreeMap::new();
    for line in std::fs::read_to_string(environment_file(target))?.lines() {
        let (variable, source) = line
            .split_once('=')
            .ok_or_else(|| io::Error::other(format!("invalid component entry: {line}")))?;
        let staged = output.join(format!("{variable}.wasm"));
        if Path::new(source) != staged {
            copy(Path::new(source), &staged)?;
        }
        let inputs = match supplied.iter().find(|base| base.variable == variable) {
            Some(base) => base.inputs.clone(),
            None => component_inputs(checkout, Path::new(source), &shared)?,
        };
        components.insert(
            variable.to_owned(),
            Component {
                file: digest(&std::fs::read(&staged)?),
                inputs,
            },
        );
    }
    Ok(components)
}

/// Build the browser graph in the checkout and stage it in `output`.
///
/// # Errors
/// Returns the build commands that failed, so no shard has to build the graph again.
fn stage_browser_graph(cargo: &Path, checkout: &Path, output: &Path) -> io::Result<()> {
    let staged = output.join(browser_graph::STAGED);
    remove_directory(&staged)?;
    browser_graph::build(checkout, cargo.as_os_str())?;
    browser_graph::stage(&checkout.join(browser_graph::TARGET), &staged)
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

/// Run the built example so it writes its component paths to the environment file of `target`
/// and its compiled code under that build's cache home. It takes the `supplied` components from
/// its environment instead of building them.
fn run_example(
    cargo: &Path,
    checkout: &Path,
    example: &Path,
    target: &Path,
    supplied: &[Supplied],
) -> io::Result<()> {
    let mut run = Command::new(example);
    run.current_dir(checkout)
        .env("CARGO", cargo)
        .env("NEXTEST_ENV", environment_file(target))
        .env("XDG_CACHE_HOME", cache_home(target));
    for name in inherited_components(std::env::vars_os().map(|(name, _)| name)) {
        run.env_remove(name);
    }
    run.envs(supplied.iter().map(|base| (&base.variable, &base.path)));
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
    let manifest = read_manifest(&output)?;
    manifest.shared.verify(checkout, &output)?;
    manifest.tool.verify(checkout, &output)?;
    if FIXED_VARIABLES
        .iter()
        .any(|variable| !manifest.components.contains_key(*variable))
    {
        return Err(at(&output, "missing prepared author or context component"));
    }
    let mut lines = Vec::new();
    for (variable, expected) in &manifest.components {
        validate_variable(variable).map_err(|error| at(&output, &error))?;
        expected.inputs.verify(checkout, &output)?;
        let path = output.join(format!("{variable}.wasm"));
        verify(&path, &expected.file)?;
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

/// Install the browser graph of the bundle in `output` at `target`.
///
/// # Errors
/// Returns a missing or stale bundle, or a failed copy.
pub(crate) fn install_browser_graph(
    checkout: &Path,
    output: &Path,
    target: &Path,
) -> io::Result<()> {
    read_manifest(output)?.browser.verify(checkout, output)?;
    browser_graph::install(&output.join(browser_graph::STAGED), target)
}

/// Read the manifest of the bundle in `output`.
fn read_manifest(output: &Path) -> io::Result<Manifest> {
    let path = output.join(MANIFEST);
    serde_json::from_slice(&std::fs::read(&path).map_err(|error| at(&path, &error))?)
        .map_err(|error| at(&path, &error))
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
pub(crate) fn remove_directory(path: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// An error that names `path`, which the bare operating-system error omits.
fn at(path: &Path, reason: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{}: {reason}", path.display()))
}
