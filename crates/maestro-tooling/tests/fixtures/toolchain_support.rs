#![cfg(test)]

use std::{ffi::OsStr, process::Command};

/// Strips runner instrumentation and selects the recorded build toolchain when present.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
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
    for key in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "LLVM_PROFILE_FILE",
        "CARGO_LLVM_COV",
        "CARGO_LLVM_COV_TARGET_DIR",
        "CARGO_LLVM_COV_BUILD_DIR",
        "CARGO_LLVM_COV_SHOW_ENV",
    ] {
        command.env_remove(key);
    }
    if let Some(channel) = channel() {
        command.env("RUSTUP_TOOLCHAIN", channel);
    }
    command
}

/// Keep a fixture compiler's Cargo state inside its supplied workspace.
///
/// # Errors
/// Returns an error when the private home cannot be created.
pub fn fixture_command(
    program: impl AsRef<OsStr>,
    root: &std::path::Path,
    _sources: Option<&std::path::Path>,
) -> std::io::Result<Command> {
    let home = root.join("cargo-home");
    std::fs::create_dir_all(&home)?;
    let mut command = command(program);
    command.current_dir(root).env("CARGO_HOME", home);
    Ok(command)
}

/// The resolved toolchain that compiled this test, independent of runtime overrides.
pub fn channel() -> Option<&'static str> {
    option_env!("RUSTUP_TOOLCHAIN")
}

/// Write the complete supplied toolchain declaration into a fixture root.
pub fn declare_toolchain(root: &std::path::Path, declaration: &str) -> std::io::Result<()> {
    std::fs::write(root.join("rust-toolchain.toml"), declaration)
}
