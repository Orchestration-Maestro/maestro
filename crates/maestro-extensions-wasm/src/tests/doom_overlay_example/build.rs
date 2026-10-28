//! Compose the shipped author component with a controlled MIT engine.
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

/// Builds the controlled component once per process.
///
/// # Errors
/// Returns a compiler, assembler or composition failure.
pub fn component() -> Result<PathBuf, String> {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    BUILT.get_or_init(build).clone()
}

/// Assembles the pixel engine and composes it with the real author component.
fn build() -> Result<PathBuf, String> {
    let author = super::super::build::component("doom_overlay", "MAESTRO_DOOM_OVERLAY_COMPONENT")?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join("src/tests/doom_overlay_example/engine.wat");
    let output = author
        .parent()
        .ok_or("missing component directory")?
        .join(format!("doom-overlay-stub-{}", std::process::id()));
    std::fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let engine = output.join("engine.wasm");
    run(Command::new("wasm-tools")
        .arg("parse")
        .arg(source)
        .arg("-o")
        .arg(&engine))?;
    run(Command::new("wasm-tools").arg("validate").arg(&engine))?;
    let composed = output.join("doom_overlay.wasm");
    run(Command::new("wasm-tools")
        .arg("compose")
        .arg(author)
        .arg("-d")
        .arg(engine)
        .arg("-o")
        .arg(&composed))?;
    Ok(composed)
}

/// Reports a tool's complete diagnostic without accepting partial output.
fn run(command: &mut Command) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("{command:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
