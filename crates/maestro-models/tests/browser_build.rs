#![cfg(not(target_arch = "wasm32"))]

use std::process::Command;

fn wasm_build_command() -> Command {
    let mut command = Command::new(env!("CARGO"));
    // The wasm child build must not inherit host instrumentation.
    command
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    command
}

#[test]
fn maestro_browser_environment_has_no_key() {
    let output = wasm_build_command()
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
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let artifact = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "browser_import_check"
        })
        .flat_map(|message| message["filenames"].as_array().unwrap().clone())
        .filter_map(|filename| filename.as_str().map(str::to_owned))
        .find(|filename| {
            std::path::Path::new(filename)
                .extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .expect("Cargo must report the actual example wasm artifact");
    let bytes = std::fs::read(artifact).unwrap();
    assert!(bytes.starts_with(b"\0asm"));
}
