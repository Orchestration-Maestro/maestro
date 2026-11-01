//! The Cargo build and artifact discovery shared by author component checks.
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../../../../maestro-tooling/src/example_files/components.rs"]
/// Source marker traversal shared with component preparation.
pub mod components;

/// Uses a prepared example or builds the selected component in the host test output.
///
/// # Errors
/// Returns source-discovery failure, Cargo's diagnostic or a missing-component error.
#[cfg(test)]
pub fn component(directory: &Path, selection: &[&str], name: &str) -> Result<PathBuf, String> {
    if selection == ["--example", name] {
        return example_components(directory, &[name])?
            .pop()
            .ok_or_else(|| "the build reported no component".to_owned());
    }
    let command = selection_command(cargo_program(), directory, selection, &target_directory()?);
    artifact(&run(command)?, name)
}

/// Finds the component of each named example, in the order given.
///
/// A supplied artifact is used as is; every other example builds in the host test output in one
/// Cargo invocation, which compiles them in parallel.
///
/// # Errors
/// Returns source-discovery failure, Cargo's diagnostic or a missing-component error.
pub fn example_components(directory: &Path, names: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut paths = names
        .iter()
        .map(|name| prepared_component(directory, name))
        .collect::<Result<Vec<_>, _>>()?;
    let missing: Vec<&str> = names
        .iter()
        .zip(&paths)
        .filter(|(_, path)| path.is_none())
        .map(|(name, _)| *name)
        .collect();
    if !missing.is_empty() {
        let command = build_command(cargo_program(), directory, &missing, &target_directory()?);
        let output = run(command)?;
        for (name, path) in names.iter().zip(&mut paths) {
            if path.is_none() {
                *path = Some(artifact(&output, name)?);
            }
        }
    }
    Ok(paths.into_iter().flatten().collect())
}

/// The Cargo command that builds every named example component in one invocation.
pub fn build_command(
    cargo: impl AsRef<OsStr>,
    directory: &Path,
    names: &[&str],
    target_directory: &Path,
) -> Command {
    let mut packages = Vec::new();
    for name in names {
        let package = components::FIXED_COMPONENTS
            .iter()
            .find(|(target, _, _)| target == name)
            .map_or(components::EXAMPLE_PACKAGE, |(_, package, _)| *package);
        if !packages.contains(&package) {
            packages.push(package);
        }
    }
    let selection: Vec<&str> = packages
        .into_iter()
        .flat_map(|package| ["--package", package])
        .chain(names.iter().flat_map(|name| ["--example", *name]))
        .collect();
    selection_command(cargo, directory, &selection, target_directory)
}

/// The Cargo command that builds the selected targets in one invocation.
///
/// The compiler wrapper is not passed on, because coverage would instrument the component target.
fn selection_command(
    cargo: impl AsRef<OsStr>,
    directory: &Path,
    selection: &[&str],
    target_directory: &Path,
) -> Command {
    let mut command = Command::new(cargo);
    command
        .current_dir(directory)
        .env_remove("RUSTC_WRAPPER")
        .args(["build", "--target", "wasm32-wasip2"])
        .args(selection)
        .arg("--target-dir")
        .arg(target_directory)
        .args(["--release", "--locked", "--message-format=json"]);
    command
}

/// The Cargo program the test run was started with.
fn cargo_program() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// Runs the one Cargo build and returns its JSON messages.
fn run(mut command: Command) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "building the author component failed (is the wasm32-wasip2 target installed? run \
             `rustup target add wasm32-wasip2`):\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Finds the component artifact that a build reported for the named target.
fn artifact(messages: &str, name: &str) -> Result<PathBuf, String> {
    messages
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| message["target"]["name"] == name)
        .flat_map(|message| message["filenames"].as_array().cloned().unwrap_or_default())
        .filter_map(|file| file.as_str().map(PathBuf::from))
        .find(|file| {
            file.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .ok_or_else(|| format!("the build reported no component for {name}"))
}

/// Looks up a supplied artifact through the same source markers that preparation visits.
fn prepared_component(directory: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    if let Some((_, _, variable)) = components::FIXED_COMPONENTS
        .iter()
        .find(|(example, _, _)| *example == name)
    {
        return Ok(std::env::var_os(variable).map(PathBuf::from));
    }
    let mut path = None;
    components::visit(
        &[
            directory.join("examples"),
            directory.join("../../examples/extensions"),
        ],
        |target, variable| {
            if target == name && path.is_none() {
                path = std::env::var_os(variable).map(PathBuf::from);
            }
            Ok(())
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(path)
}

/// Finds the target directory above the running test or preparation example.
fn target_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    executable
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate the test build's target directory".into())
}
