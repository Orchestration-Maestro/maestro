//! Builds the author component once per test run.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Build target of author components.
const TARGET: &str = "wasm32-wasip2";

/// The component built from the shared author extension.
///
/// The build is the one `just extension-author-component` runs. It happens once and every
/// scenario shares the artifact. The compiler wrapper is not passed on, because the coverage wrapper
/// would instrument the component target.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
pub fn author_component() -> Result<&'static Path, String> {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    BUILT.get_or_init(build).as_deref().map_err(Clone::clone)
}

/// Runs the author build and finds the artifact it reports.
fn build() -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("RUSTC_WRAPPER")
        .args(["build", "--example", "author_component", "--target", TARGET])
        .args(["--release", "--locked", "--message-format=json"])
        .output()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "building the author component failed (is the {TARGET} target installed? run \
             `rustup target add {TARGET}`):\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| message["target"]["name"] == "author_component")
        .flat_map(|message| message["filenames"].as_array().cloned().unwrap_or_default())
        .filter_map(|file| file.as_str().map(PathBuf::from))
        .find(|file| {
            file.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .ok_or_else(|| "the build reported no component".to_owned())
}
