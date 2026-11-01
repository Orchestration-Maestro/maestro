#![cfg(not(target_arch = "wasm32"))]

use std::process::Command;

/// Strip host instrumentation while retaining the ordinary compiler cache.
fn wasm_build_command() -> Command {
    let mut command = Command::new(env!("CARGO"));
    if std::env::var_os("__CARGO_LLVM_COV_RUSTC_WRAPPER").is_some() {
        if let Some(wrapper) = std::env::var_os("__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING") {
            command.env("RUSTC_WRAPPER", wrapper);
        } else {
            command.env_remove("RUSTC_WRAPPER");
        }
    }
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .starts_with("__CARGO_LLVM_COV_RUSTC_WRAPPER")
        {
            command.env_remove(key);
        }
    }
    command
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
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
