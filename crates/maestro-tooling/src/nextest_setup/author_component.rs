//! Runs the component-discovery example before the tests that read its components.
//!
//! Nextest supplies its active artifact directory in the platform library search path. A run
//! from a prepared bundle installs the bundle's example there; any other run uses the test
//! build's own example, or builds one.
//!
//! On Linux the example warms compiled code in a cache that only this run uses, and the run's
//! tests inherit that cache through `XDG_CACHE_HOME`. Wasmtime prunes its user cache by age
//! and size from every process that uses it, so on a host that runs several builds the shared
//! cache can evict the code this setup warmed before the tests that read it have run. macOS
//! and Windows ignore the variable, so there the example warms the engine's own cache.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use super::normalize::{self, Outcome, archive_signalled};
use super::{export, prepared_bundle};
use crate::exit_code;
use crate::test_setup::{self, EXAMPLE, example_arguments, executable_name};
use crate::workspace::Workspace;

/// The sources the setup example is built from, beside the shared build and discovery
/// directories listed below; the example is outdated once any of them is newer.
const SOURCES: [&str; 5] = [
    "crates/maestro-extensions-wasm/tests/support/prepare_author_component.rs",
    "crates/maestro-extensions-wasm/src/tests/build.rs",
    "crates/maestro-tooling/src/example_files.rs",
    "crates/maestro-extensions-wasm/Cargo.toml",
    "Cargo.lock",
];

/// The directories whose `.rs` files the setup example is built from.
const SOURCE_DIRECTORIES: [&str; 2] = [
    "crates/maestro-extensions-wasm/src/tests/build",
    "crates/maestro-tooling/src/example_files",
];

/// Prepare the components with a staged or built example and run it, then export the cache
/// home the example warmed so the tests read the same compiled code.
///
/// # Errors
/// Returns an unresolved artifact directory, a workspace without one component crate, a stale
/// or missing bundle, a failed build or install, or a setup example that cannot start. A setup
/// example that fails keeps its own status.
pub(super) fn run(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let directory = artifact_directory()?;
    let cache_home = test_setup::run_cache_home(&directory);
    let status = if let Some(bundle) = prepared_bundle(checkout) {
        run_prepared(cargo, checkout, &bundle, &directory, &cache_home)?
    } else {
        let workspace = Workspace::read(cargo, checkout)?;
        // The example is given the component crate, so a workspace without one fails here,
        // before a build.
        workspace.component_crate()?;
        run_local(cargo, &workspace, &directory, &cache_home)?
    };
    // The example and the installation rewrite the file, so the entry follows them.
    let mut entry = OsString::from("XDG_CACHE_HOME=");
    entry.push(cache_home);
    export(&entry)?;
    Ok(status)
}

/// Install the bundle's example, give the executables one time and run the example on the
/// bundle's components. The staged example is as new as the archive's executables, so it is
/// never outdated.
fn run_prepared(
    cargo: &Path,
    checkout: &Path,
    bundle: &Path,
    directory: &Path,
    cache_home: &Path,
) -> io::Result<ExitCode> {
    let environment = environment_file()?;
    test_setup::install_bundle(cargo, checkout, bundle, directory, &environment)?;
    if normalize::normalize(directory, archive_signalled())? == Outcome::ExampleOutdated {
        return Ok(ExitCode::from(3));
    }
    let example = directory.join("examples").join(executable_name());
    let mut command = Command::new(example);
    command.env("XDG_CACHE_HOME", cache_home);
    // The example takes the prepared components from its environment, so it only warms.
    for line in std::fs::read_to_string(&environment)?.lines() {
        if let Some((name, value)) = line.split_once('=') {
            command.env(name, value);
        }
    }
    let workspace = Workspace::read(cargo, checkout)?;
    command.args(example_arguments(&workspace)?);
    Ok(exit_code(command.current_dir(checkout).status()?))
}

/// Run the test build's example while it is newer than its sources, else prepare one first.
///
/// The test build keeps the example's resolved features; a single-package build resolves fewer
/// and recompiles the engine. Running it only while it is newer than its sources keeps a
/// filtered build from running outdated preparation.
fn run_local(
    cargo: &Path,
    workspace: &Workspace,
    directory: &Path,
    cache_home: &Path,
) -> io::Result<ExitCode> {
    let profile = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let profile = if profile == "debug" { "dev" } else { profile };
    let target = directory.parent().unwrap_or(directory);
    if let Some(example) = built_example(directory)
        && fresh(workspace.root(), &example)
        && normalize::normalize(directory, archive_signalled())? == Outcome::Normalized
    {
        return start(workspace, &example, cache_home);
    }
    let example = test_setup::example_program(cargo, workspace.root(), workspace, target, profile)?;
    // Cargo may retain an outdated example; running it still allows preparation to finish.
    if normalize::normalize(directory, archive_signalled())? == Outcome::ExampleOutdated {
        eprintln!("Cargo kept the setup example; test binaries may compile components again.");
    }
    start(workspace, &example, cache_home)
}

/// Run the setup example in the checkout with this process's environment and `cache_home` as
/// its user cache, and the workspace it prepares as its arguments.
fn start(workspace: &Workspace, example: &Path, cache_home: &Path) -> io::Result<ExitCode> {
    let mut command = Command::new(example);
    command
        .current_dir(workspace.root())
        .args(example_arguments(workspace)?)
        .env("XDG_CACHE_HOME", cache_home);
    Ok(exit_code(command.status()?))
}

/// The setup example the test build left in `directory`, if any.
fn built_example(directory: &Path) -> Option<PathBuf> {
    [EXAMPLE.to_owned(), format!("{EXAMPLE}.exe")]
        .into_iter()
        .map(|name| directory.join("examples").join(name))
        .find(|path| path.is_file())
}

/// Whether `example` is newer than every source it is built from.
fn fresh(checkout: &Path, example: &Path) -> bool {
    let modified = |path: &Path| {
        std::fs::metadata(path)
            .and_then(|data| data.modified())
            .ok()
    };
    let Some(built) = modified(example) else {
        return false;
    };
    let listed = SOURCES.iter().map(|source| checkout.join(source));
    let discovered = SOURCE_DIRECTORIES
        .iter()
        .filter_map(|folder| std::fs::read_dir(checkout.join(folder)).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"));
    listed
        .chain(discovered)
        .filter_map(|source| modified(&source))
        .all(|source| built > source)
}

/// The first directory of the platform's library search path that holds test dependencies.
fn artifact_directory() -> io::Result<PathBuf> {
    let variable = if cfg!(target_os = "macos") {
        "NEXTEST_DYLD_FALLBACK_LIBRARY_PATH"
    } else if cfg!(windows) {
        "PATH"
    } else {
        "NEXTEST_LD_LIBRARY_PATH"
    };
    std::env::split_paths(&std::env::var_os(variable).unwrap_or_default())
        .find(|directory| directory.join("deps").is_dir())
        .ok_or_else(|| {
            io::Error::other(
                "Cannot resolve the nextest artifact directory from its library search path.",
            )
        })
}

/// The file nextest reads the exported variables from.
fn environment_file() -> io::Result<PathBuf> {
    std::env::var_os("NEXTEST_ENV")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("NEXTEST_ENV is not set"))
}
