//! The Cargo build and artifact discovery shared by author component checks.
use std::path::{Path, PathBuf};
use std::process::Command;

/// Builds a selected component in the host test build's target directory.
///
/// # Errors
/// Returns Cargo's diagnostic or a missing-component error.
pub fn component(directory: &Path, selection: &[&str], name: &str) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(directory)
        .env_remove("RUSTC_WRAPPER")
        .args(["build", "--target", "wasm32-wasip2"])
        .args(selection)
        .arg("--target-dir")
        .arg(target_directory()?)
        .args(["--release", "--locked", "--message-format=json"])
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

/// Finds the target directory above the running test or preparation example.
fn target_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    executable
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate the test build's target directory".into())
}
