//! Build the browser import example for the browser target and read its artifact.

use std::process::Command;

/// Build `browser_import_check` for the browser target and return the bytes of its wasm artifact.
pub fn build() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO"))
        // The wasm child build must not inherit host instrumentation.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args([
            "build",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
            "--package",
            "maestro-models",
            "--example",
            "browser_import_check",
            "--message-format=json",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let artifact = String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "browser_import_check"
        })
        .flat_map(|message| message["filenames"].as_array().cloned().unwrap_or_default())
        .filter_map(|filename| filename.as_str().map(str::to_owned))
        .find(|filename| {
            std::path::Path::new(filename)
                .extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .ok_or("Cargo must report the actual example wasm artifact")?;
    Ok(std::fs::read(artifact)?)
}
