//! Runs the component-discovery example before the tests that read its components.
//!
//! Nextest supplies its active artifact directory in the platform library search path. A run
//! from a prepared bundle installs the bundle's example there; any other run uses the test
//! build's own example, or builds one.
//!
//! On every platform the example warms compiled code in a cache that only this run uses: it
//! sits below an agent directory the run owns, and the run's tests inherit that directory
//! through `MAESTRO_CODING_AGENT_DIR`. Wasmtime prunes its cache by age and size from every
//! process that uses it, so on a host that runs several builds a shared cache can evict the
//! code this setup warmed before the tests that read it have run.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use super::normalize::{self, Outcome, archive_signalled};
use super::{export, prepared_bundle};
use crate::example_files::EXAMPLE_PACKAGE;
use crate::exit_code;
use crate::test_setup::{self, EXAMPLE, example_arguments, executable_name};
use crate::workspace::{FIXTURES, Workspace};

/// Prepare the components with a staged or built example and run it, then export the agent
/// directory whose cache the example warmed so the tests read the same compiled code.
///
/// # Errors
/// Returns an unresolved artifact directory, a workspace without one component crate, a stale
/// or missing bundle, a failed build or install, or a setup example that cannot start. A setup
/// example that fails keeps its own status.
pub(super) fn run(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let directory = artifact_directory()?;
    let agent_directory = test_setup::run_agent_directory(&directory);
    let status = if let Some(bundle) = prepared_bundle(checkout) {
        run_prepared(cargo, checkout, &bundle, &directory, &agent_directory)?
    } else {
        let workspace = Workspace::read(cargo, checkout)?;
        // The example is given the component crate, so a workspace without one fails here,
        // before a build.
        workspace.component_crate()?;
        run_local(cargo, &workspace, &directory, &agent_directory)?
    };
    // The example and the installation rewrite the file, so the entry follows them.
    let mut entry = OsString::from(format!("{}=", test_setup::AGENT_DIRECTORY));
    entry.push(agent_directory);
    export(&entry)?;
    Ok(status)
}

/// Install the bundle's example, give the executables one time, install the compiled code for
/// that time and run the example on the bundle's components. The staged example is as new as
/// the archive's executables, so it is never outdated.
fn run_prepared(
    cargo: &Path,
    checkout: &Path,
    bundle: &Path,
    directory: &Path,
    agent_directory: &Path,
) -> io::Result<ExitCode> {
    let environment = environment_file()?;
    let compiled = test_setup::install_bundle(cargo, checkout, bundle, directory, &environment)?;
    if normalize::normalize(directory, archive_signalled())? == Outcome::ExampleOutdated {
        return Ok(ExitCode::from(3));
    }
    // The engine names the cache namespace for the example's time, which normalization just set.
    if let Some(compiled) = compiled {
        compiled.install()?;
    }
    let example = directory.join("examples").join(executable_name());
    let mut command = Command::new(example);
    command.env(test_setup::AGENT_DIRECTORY, agent_directory);
    for (variable, _, _) in crate::search_tools::PROGRAMS {
        command.env_remove(variable);
    }
    // The example takes the prepared components from its environment, so it only warms.
    for line in std::fs::read_to_string(&environment)?.lines() {
        if let Some((name, value)) = line.split_once('=') {
            command.env(name, value);
        }
    }
    let workspace = Workspace::read(cargo, checkout)?;
    command
        .args(example_arguments(&workspace)?)
        .arg("--prepared");
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
    agent_directory: &Path,
) -> io::Result<ExitCode> {
    let profile = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let profile = if profile == "debug" { "dev" } else { profile };
    let target = directory.parent().unwrap_or(directory);
    if let Some(example) = built_example(directory)
        && fresh(workspace, &example)
        && normalize::normalize(directory, archive_signalled())? == Outcome::Normalized
    {
        return start(workspace, &example, agent_directory);
    }
    let example = test_setup::example_program(cargo, workspace.root(), workspace, target, profile)?;
    // Cargo may retain an outdated example; running it still allows preparation to finish.
    if normalize::normalize(directory, archive_signalled())? == Outcome::ExampleOutdated {
        eprintln!("Cargo kept the setup example; test binaries may compile components again.");
    }
    start(workspace, &example, agent_directory)
}

/// Run the setup example in the checkout with this process's environment and `agent_directory` as
/// its agent directory, and the workspace it prepares as its arguments.
fn start(workspace: &Workspace, example: &Path, agent_directory: &Path) -> io::Result<ExitCode> {
    let mut command = Command::new(example);
    command
        .current_dir(workspace.root())
        .args(example_arguments(workspace)?)
        .env(test_setup::AGENT_DIRECTORY, agent_directory);
    Ok(exit_code(command.status()?))
}

/// The setup example the test build left in `directory`, if any.
fn built_example(directory: &Path) -> Option<PathBuf> {
    [EXAMPLE.to_owned(), format!("{EXAMPLE}.exe")]
        .into_iter()
        .map(|name| directory.join("examples").join(name))
        .find(|path| path.is_file())
}

/// Whether `example` is newer than every workspace file it is built from.
///
/// The example is compiled from the example package's sources, the tooling and the lock.
///
/// A workspace that holds neither package has only the lock to compare, and keeps an installed
/// example.
fn fresh(workspace: &Workspace, example: &Path) -> bool {
    let modified = |path: &Path| {
        std::fs::metadata(path)
            .and_then(|data| data.modified())
            .ok()
    };
    let Some(built) = modified(example) else {
        return false;
    };
    let component = workspace.member(EXAMPLE_PACKAGE);
    let tooling = workspace.member("maestro-tooling");
    let listed = [
        component
            .as_ref()
            .map(|owner| owner.join(FIXTURES).join(format!("{EXAMPLE}.rs"))),
        component
            .as_ref()
            .map(|owner| owner.join("src/tests/build.rs")),
        component.as_ref().map(|owner| owner.join("Cargo.toml")),
        tooling
            .as_ref()
            .map(|owner| owner.join("src/example_files.rs")),
        tooling
            .as_ref()
            .map(|owner| owner.join("src/search_tools.rs")),
        tooling
            .as_ref()
            .map(|owner| owner.join("src/search_tools/resolution.rs")),
        Some(workspace.root().join("Cargo.lock")),
    ]
    .into_iter()
    .flatten();
    let discovered = [
        component
            .as_ref()
            .map(|owner| owner.join("src/tests/build")),
        tooling
            .as_ref()
            .map(|owner| owner.join("src/example_files")),
    ]
    .into_iter()
    .flatten()
    .flat_map(std::fs::read_dir)
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
