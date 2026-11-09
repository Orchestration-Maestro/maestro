//! Stages the reusable test-setup files in one job and installs them in another.
//!
//! `prepare` runs the component-discovery example in a separate build and stages its component
//! files, the code it compiled, the built example, the browser graph the browser author tests
//! share, the registry units the cold browser bootstrap starts from, this command and a content
//! manifest. The manifest records the inputs of each artifact on its own, so a change reaches
//! only the artifacts that read it. The first earlier bundle in the path list
//! `TEST_PREPARE_BASE` that fits supplies every component whose inputs are unchanged, and its
//! compiled code, so only the rest are built.
//! `install` stages this command alone, for runs without a bundle. The staged command's
//! `author-component` setup validates the components and the example against the checkout,
//! installs the example beside the extracted test executables and the compiled code in the
//! run's agent directory, and writes the component paths for nextest. The `browser-author` setup
//! validates and installs the browser graph, and the `workspace-sources` setup installs the
//! registry units beside the sources they were compiled against.

use std::collections::BTreeMap;
use std::env::consts::EXE_EXTENSION;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::argument;
use super::copy_assets::copy_tree;
use super::example_files::{EXAMPLE_PACKAGE, FIXED_COMPONENTS, Fixed, validate_variable};
use super::nextest_setup::{browser_graph, fixture_sources};
use super::workspace::Workspace;
use base::{Base, Supplied};
use compiled_code::CompiledCode;
use inputs::Inputs;
use maestro_path::normalize_native;
use plan::Artifacts;
use program::{Program, Source, Target};

mod base;
pub(crate) mod cargo_checks;
mod compiled_code;
mod examples;
pub(crate) mod executable;
mod inputs;
pub(crate) mod native_fixture;
mod plan;
mod program;
#[cfg(test)]
mod tests;

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
/// The packages the setup example and the development tool that runs it are built from.
const TOOL_PACKAGES: [&str; 2] = [EXAMPLE_PACKAGE, "maestro-tooling"];
/// The packages the browser graph is built from.
const BROWSER_PACKAGES: [&str; 2] = [browser_graph::PACKAGE, "maestro-tooling"];
/// The package whose cold browser bootstrap test starts from the staged seed.
const SEED_PACKAGE: &str = "maestro-tooling";
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
    /// The directory of the workspace sources the staged cold bootstrap seed was compiled
    /// against, when the workspace holds the package whose test starts from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace_sources: Option<PathBuf>,
    /// Each component file, by the variable that exports it.
    components: BTreeMap<String, Component>,
    /// Digest of the staged setup example, when the workspace has a component crate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    example: Option<String>,
    /// The code the example compiled, when the engine keeps it in its cache.
    compiled: Option<CompiledCode>,
    /// Digest of the staged executable, when the workspace builds it elsewhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    executable: Option<executable::Prepared>,
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

/// The components prepared without a source marker whose packages this workspace builds.
///
/// A component whose package the workspace only depends on is built by Cargo outside its tree,
/// so preparation leaves it to that package's own repository.
pub(crate) fn fixed_components(workspace: &Workspace) -> Vec<Fixed> {
    FIXED_COMPONENTS
        .iter()
        .copied()
        .filter(|(_, package, _)| workspace.member(package).is_some())
        .collect()
}

/// Build and run the setup example separately, then stage its output in `output`.
/// Components and compiled code come from the first bundle in the path
/// list `TEST_PREPARE_BASE` names that was prepared from the same shared inputs.
/// A workspace without a component crate has no components and no example to stage.
fn prepare(cargo: &Path, checkout: &Path, output: &Path, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(output)?;
    let workspace = Workspace::read(cargo, checkout)?;
    let component = workspace.find_component_crate()?;
    let artifacts = Artifacts::read(checkout, &workspace, component.as_ref())?;
    let examples = if component.is_some() {
        examples::stage(checkout, &workspace, &artifacts.shared, output, target)?
    } else {
        remove(&output.join(MANIFEST))?;
        examples::Examples::default()
    };
    native_fixture::prepare(cargo, checkout, &workspace, output, target)?;
    cargo_checks::prepare(
        cargo,
        checkout,
        &workspace,
        output,
        &target.join("cargo-checks"),
    )?;
    let workspace_sources = stage_browser(cargo, checkout, &workspace, output)?;
    let executable = executable::stage(cargo, checkout, &workspace, output, target)?;
    check_unchanged(checkout, output, &artifacts, &examples.components)?;
    stage_tool(output)?;
    let (reused, built) = (examples.reused, examples.built);
    let manifest = Manifest {
        shared: artifacts.shared,
        tool: artifacts.tool,
        browser: artifacts.browser,
        workspace_sources,
        components: examples.components,
        example: examples.example,
        compiled: examples.compiled,
        executable,
    };
    std::fs::write(output.join(MANIFEST), serde_json::to_vec(&manifest)?)?;
    writeln!(io::stdout(), "components: {reused} reused, {built} built")
}

/// Check that `checkout` still holds what every artifact and component staged in `output` was
/// prepared from.
///
/// # Errors
/// Returns inputs that changed while the preparation ran, or a failed listing or read.
fn check_unchanged(
    checkout: &Path,
    output: &Path,
    artifacts: &Artifacts,
    components: &BTreeMap<String, Component>,
) -> io::Result<()> {
    let mut unchanged = artifacts.is_current(checkout)?;
    for component in components.values() {
        unchanged &= component.inputs.is_current(checkout)?;
    }
    if unchanged {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "inputs changed during preparation: {}",
            output.display()
        )))
    }
}

/// Read the first bundle in `TEST_PREPARE_BASE` prepared from the same shared inputs.
fn read_base(checkout: &Path, shared: &Inputs) -> Option<Base> {
    std::env::var_os(BASE).and_then(|list| {
        std::env::split_paths(&list)
            .filter(|name| !name.as_os_str().is_empty())
            .find_map(|name| Base::read(&checkout.join(name), shared))
    })
}

/// The file the setup example writes its component paths to.
fn environment_file(target: &Path) -> PathBuf {
    target.join("nextest-env")
}

/// The target-local agent directory supplied to the setup example, below which the engine keeps
/// its compiled-code cache.
fn preparation_agent(target: &Path) -> PathBuf {
    target.join("agent-home")
}

/// Build the browser graph in the checkout and stage it in `output` when the workspace holds
/// the browser package, then stage the cold bootstrap seed when the workspace holds its package,
/// and return the directory of the workspace sources the seed was compiled against.
///
/// # Errors
/// Returns the build commands that failed, so no shard has to build the graph or the seed again.
fn stage_browser(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
) -> io::Result<Option<PathBuf>> {
    let staged = output.join(browser_graph::STAGED);
    remove_directory(&staged)?;
    if workspace.member(browser_graph::PACKAGE).is_some() {
        browser_graph::build(checkout, cargo.as_os_str())?;
        browser_graph::stage(&checkout.join(browser_graph::TARGET), &staged)?;
    }
    if workspace.member(SEED_PACKAGE).is_none() {
        return Ok(None);
    }
    // The sources are vendored offline, from the packages a shard's job fetches before its tests.
    let fetched = Command::new(cargo)
        .current_dir(checkout)
        .args(["fetch", "--locked"])
        .status()?;
    if !fetched.success() {
        return Err(io::Error::other(format!("cargo fetch failed: {fetched}")));
    }
    let directory = fixture_sources::stage(cargo, checkout, output)?;
    Ok(Some(normalize_native(&directory)))
}

/// Copy this command into `output`, where the nextest setup entries run it.
fn stage_tool(output: &Path) -> io::Result<()> {
    copy(
        &std::env::current_exe()?,
        &output.join(executable(EXPORTER, EXE_EXTENSION)),
    )
}

/// Build the setup example in `target` with the Cargo `profile`, or install it when the
/// workspace reaches its package through a locked dependency.
pub(crate) fn build_example(
    cargo: &Path,
    checkout: &Path,
    target: &Path,
    profile: &str,
) -> io::Result<()> {
    let status = Command::new(cargo)
        .current_dir(checkout)
        .args(["build", "--quiet", "--locked", "-p", EXAMPLE_PACKAGE])
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

/// The executable the setup example was built or installed as.
pub(crate) fn example_program(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    target: &Path,
    profile: &str,
) -> io::Result<PathBuf> {
    let program = Program {
        package: EXAMPLE_PACKAGE,
        target: Target::Example(EXAMPLE),
    };
    let source = program::source(checkout, workspace, EXAMPLE_PACKAGE)?.ok_or_else(|| {
        io::Error::other(format!(
            "{EXAMPLE_PACKAGE} is neither a workspace crate nor a locked dependency"
        ))
    })?;
    if let Source::Member = source {
        build_example(cargo, checkout, target, profile)?;
        // Cargo names the development profile's output directory `debug`.
        let directory = if profile == "dev" { "debug" } else { profile };
        return Ok(target
            .join(directory)
            .join("examples")
            .join(executable_name()));
    }
    program::install(
        cargo,
        checkout,
        &source,
        &program,
        &target.join("installed"),
    )
}

/// The arguments that tell the setup example which workspace to prepare: the workspace root,
/// the component crate, its package and the fixed components this workspace builds.
///
/// # Errors
/// Returns a workspace without one component crate.
pub(crate) fn example_arguments(workspace: &Workspace) -> io::Result<Vec<OsString>> {
    let component = workspace.component_crate()?;
    let fixed = fixed_components(workspace);
    let names: Vec<&str> = fixed.iter().map(|(name, _, _)| *name).collect();
    Ok(vec![
        workspace.root().as_os_str().to_owned(),
        component.directory.into_os_string(),
        component.package.into(),
        names.join(",").into(),
    ])
}

/// The staged name of the setup example.
pub(crate) fn executable_name() -> PathBuf {
    executable(EXAMPLE, EXE_EXTENSION)
}

/// Run the built example so it writes its component paths to the environment file of `target`
/// and its compiled code under that build's agent directory. It takes the `supplied` components from
/// its environment instead of building them, and finds the workspace it prepares, the component
/// crate and the fixed components this workspace builds through its arguments.
fn run_example(
    cargo: &Path,
    workspace: &Workspace,
    example: &Path,
    target: &Path,
    supplied: &[Supplied],
) -> io::Result<()> {
    let mut run = preparation_command(workspace.root(), example, target);
    run.args(example_arguments(workspace)?);
    run.env("CARGO", cargo)
        .env("NEXTEST_ENV", environment_file(target));
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

/// Configure the setup example with the same cache isolation for preparation and path queries.
fn preparation_command(checkout: &Path, example: &Path, target: &Path) -> Command {
    let mut command = Command::new(example);
    command
        .current_dir(checkout)
        .env(AGENT_DIRECTORY, preparation_agent(target));
    command
}

/// The variable that names the agent directory, below which the engine keeps compiled components.
pub(crate) const AGENT_DIRECTORY: &str = "MAESTRO_CODING_AGENT_DIR";

/// The directory beside the profile directory that a test run uses as its agent directory. It is
/// separate from the preparation agent directory, which each preparation deletes.
const RUN_AGENT: &str = "run-agent";

/// The agent directory a test run shares, beside its profile directory.
pub(crate) fn run_agent_directory(directory: &Path) -> PathBuf {
    directory.parent().unwrap_or(directory).join(RUN_AGENT)
}

/// Validate the bundle in `output`, install its example in `directory` and its compiled code in
/// that run's agent directory, and write the component paths to `environment`. Nothing is installed
/// or written unless every check passes.
///
/// # Errors
/// Returns a missing, stale or incomplete bundle, or a failed copy.
pub(crate) fn install_bundle(
    cargo: &Path,
    checkout: &Path,
    output: &Path,
    directory: &Path,
    environment: &Path,
) -> io::Result<()> {
    export(cargo, checkout, output, directory, environment)
}

/// Validate and install the bundle, and install its compiled code in the cache directory the
/// installed example reports for this run's agent directory.
fn export(
    cargo: &Path,
    checkout: &Path,
    output: &Path,
    directory: &Path,
    environment: &Path,
) -> io::Result<()> {
    let output = maestro_realpath::canonicalize(output).map_err(|error| at(output, &error))?;
    if output.to_string_lossy().contains(['\n', '\r']) {
        return Err(at(&output, "the path must stay on one line"));
    }
    let manifest = read_manifest(&output)?;
    manifest.shared.verify(checkout, &output)?;
    manifest.tool.verify(checkout, &output)?;
    let workspace = Workspace::read(cargo, checkout)?;
    if fixed_components(&workspace)
        .iter()
        .any(|(_, _, variable)| !manifest.components.contains_key(*variable))
    {
        return Err(at(&output, "missing prepared fixed component"));
    }
    let mut lines = Vec::new();
    for (variable, expected) in &manifest.components {
        validate_variable(variable).map_err(|error| at(&output, &error))?;
        expected.inputs.verify(checkout, &output)?;
        let path = output.join(format!("{variable}.wasm"));
        verify(&path, &expected.file)?;
        lines.push(format!("{variable}={}\n", path.display()));
    }
    let example = manifest
        .example
        .as_deref()
        .ok_or_else(|| at(&output, "the bundle holds no setup example"))?;
    let name = executable_name();
    let staged = output.join(&name);
    verify(&staged, example)?;
    let compiled = output.join(COMPILED);
    if let Some(code) = &manifest.compiled {
        code.verify(&compiled)?;
    }
    let installed = directory.join("examples").join(name);
    std::fs::create_dir_all(directory.join("examples"))?;
    copy(&staged, &installed)?;
    if let Some(code) = &manifest.compiled {
        let cache_directory = compiled_code::directory(
            Command::new(&installed)
                .current_dir(checkout)
                .env(AGENT_DIRECTORY, run_agent_directory(directory)),
        )?;
        code.install(&compiled, &cache_directory, &installed)?;
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

/// Copy the cold bootstrap seed of the bundle in `output` to `seed` when the bundle compiled it
/// against the workspace sources in `directory`, and report whether it did.
///
/// # Errors
/// Returns an unreadable manifest or a failed copy.
pub(crate) fn install_seed(output: &Path, directory: &Path, seed: &Path) -> io::Result<bool> {
    if read_manifest(output)?.workspace_sources != Some(normalize_native(directory)) {
        return Ok(false);
    }
    remove_directory(seed)?;
    copy_tree(&output.join(fixture_sources::STAGED), seed)?;
    Ok(true)
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
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The native file name of a Cargo executable, keeping the target name separate.
pub(crate) fn executable(name: &str, extension: &str) -> PathBuf {
    Path::new(name).with_extension(extension)
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
