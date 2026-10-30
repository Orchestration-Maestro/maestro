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
pub fn component(directory: &Path, selection: &[&str], name: &str) -> Result<PathBuf, String> {
    if selection == ["--example", name]
        && let Some(path) = prepared_component(directory, name)?
    {
        return Ok(path);
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = build_command(cargo, directory, &[name], &target_directory()?)
        .output()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "building the author component failed (is the wasm32-wasip2 target installed? run \
             `rustup target add wasm32-wasip2`):\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8_lossy(&output.stdout)
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
        .ok_or_else(|| "the build reported no component".to_owned())
}

/// The Cargo command that builds every named example component in one invocation.
///
/// The compiler wrapper is not passed on, because coverage would instrument the component target.
pub fn build_command(
    cargo: impl AsRef<OsStr>,
    directory: &Path,
    names: &[&str],
    target_directory: &Path,
) -> Command {
    let mut command = Command::new(cargo);
    command
        .current_dir(directory)
        .env_remove("RUSTC_WRAPPER")
        .args(["build", "--target", "wasm32-wasip2"]);
    for name in names.iter().take(1) {
        command.args(["--example", name]);
    }
    command.arg("--target-dir").arg(target_directory).args([
        "--release",
        "--locked",
        "--message-format=json",
    ]);
    command
}

/// Looks up a supplied artifact through the same source markers that preparation visits.
fn prepared_component(directory: &Path, name: &str) -> Result<Option<PathBuf>, String> {
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
