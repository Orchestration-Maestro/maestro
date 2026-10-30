//! Runs the component-discovery example before the tests that read its components.
//!
//! Nextest supplies its active artifact directory in the platform library search path. A run
//! from a prepared bundle installs the bundle's example there; any other run uses the test
//! build's own example, or builds one.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use super::normalize::{self, Outcome, archive_signalled};
use super::{cargo, nonempty};
use crate::exit_code;
use crate::test_setup::{self, EXAMPLE, MANIFEST};

/// The bundle directory a run reads when it names none.
const BUNDLE: &str = "target/test-setup";

/// The sources the setup example is built from, beside the shared build and discovery
/// directories listed below; the example is outdated once any of them is newer.
const SOURCES: [&str; 5] = [
    "crates/maestro-extensions-wasm/examples/prepare_author_component.rs",
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

/// Prepare the components with a staged or built example and run it.
///
/// # Errors
/// Returns an unresolved artifact directory, a stale or missing bundle, a failed build, or
/// a setup example that cannot start. A setup example that fails keeps its own status.
pub(super) fn run(checkout: &Path) -> io::Result<ExitCode> {
    let directory = artifact_directory()?;
    let named = nonempty("TEST_PREPARE_OUTPUT").map(PathBuf::from);
    let archived = std::env::var_os("TEST_ARCHIVE").is_some_and(|value| value == "1");
    let prepared = archived
        || named
            .as_ref()
            .is_some_and(|output| checkout.join(output).join(MANIFEST).is_file());
    if prepared {
        let bundle = checkout.join(named.unwrap_or_else(|| BUNDLE.into()));
        return run_prepared(checkout, &bundle, &directory);
    }
    run_local(checkout, &directory)
}

/// Install the bundle's example, give the executables one time and run the example on the
/// bundle's components. The staged example is as new as the archive's executables, so it is
/// never outdated.
fn run_prepared(checkout: &Path, bundle: &Path, directory: &Path) -> io::Result<ExitCode> {
    let environment = environment_file()?;
    test_setup::install_bundle(checkout, bundle, directory, &environment)?;
    if normalize::normalize(checkout, directory, archive_signalled())? == Outcome::ExampleOutdated {
        return Ok(ExitCode::from(3));
    }
    let example = directory.join("examples").join(EXAMPLE);
    let mut command = Command::new(example);
    // The example takes the prepared components from its environment, so it only warms.
    for line in std::fs::read_to_string(&environment)?.lines() {
        if let Some((name, value)) = line.split_once('=') {
            command.env(name, value);
        }
    }
    Ok(exit_code(command.current_dir(checkout).status()?))
}

/// Run the test build's example while it is newer than its sources, else build it first.
///
/// The test build keeps the example's resolved features; a single-package `cargo run`
/// resolves fewer and recompiles the engine. Running it only while it is newer than its
/// sources keeps a filtered build from running outdated preparation.
fn run_local(checkout: &Path, directory: &Path) -> io::Result<ExitCode> {
    if let Some(example) = built_example(directory).filter(|example| fresh(checkout, example))
        && normalize::normalize(checkout, directory, archive_signalled())? == Outcome::Normalized
    {
        return start(checkout, &example);
    }
    build(checkout, directory)?;
    let example = built_example(directory)
        .ok_or_else(|| io::Error::other(format!("{EXAMPLE} was not built")))?;
    // Cargo ignores some library outputs that normalization counts, such as the metadata a
    // lint run writes, so the example can stay outdated. That only costs a cache miss.
    if normalize::normalize(checkout, directory, archive_signalled())? == Outcome::ExampleOutdated {
        eprintln!("Cargo kept the setup example; test binaries may compile components again.");
    }
    start(checkout, &example)
}

/// Run the setup example in the checkout with this process's environment.
fn start(checkout: &Path, example: &Path) -> io::Result<ExitCode> {
    Ok(exit_code(
        Command::new(example).current_dir(checkout).status()?,
    ))
}

/// Build the setup example in the target directory and profile of `directory`.
fn build(checkout: &Path, directory: &Path) -> io::Result<()> {
    let profile = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let profile = if profile == "debug" { "dev" } else { profile };
    let target = directory.parent().unwrap_or(directory);
    let status = Command::new(cargo())
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
