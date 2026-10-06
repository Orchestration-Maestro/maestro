use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

pub(super) fn run(root: &Path, args: Vec<OsString>) -> Result<u8, String> {
    let output = Command::new("rustup")
        .args(["which", "rustdoc"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("repository tools: resolve rustdoc: {error}"))?;
    if !output.status.success() {
        return Ok(super::status_code(output.status));
    }
    let tool = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let mut command = Command::new(tool.trim_end_matches(['\r', '\n']));
    command.args(args);
    super::cargo_target::runtime_environment(&mut command);
    // Rustdoc compiles source expressions using Cargo's package metadata.
    for name in [
        "CARGO_PKG_NAME",
        "CARGO_PKG_VERSION",
        "CARGO_PKG_VERSION_MAJOR",
        "CARGO_PKG_VERSION_MINOR",
        "CARGO_PKG_VERSION_PATCH",
        "CARGO_PKG_VERSION_PRE",
        "CARGO_PKG_AUTHORS",
        "CARGO_PKG_DESCRIPTION",
        "CARGO_PKG_HOMEPAGE",
        "CARGO_PKG_REPOSITORY",
        "CARGO_PKG_LICENSE",
        "CARGO_PKG_LICENSE_FILE",
        "CARGO_PKG_RUST_VERSION",
        "CARGO_PKG_README",
        "CARGO_CRATE_NAME",
        "CARGO_BIN_NAME",
        "OUT_DIR",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    if let Some(flags) = std::env::var_os("RUSTDOCFLAGS") {
        command.env("RUSTDOCFLAGS", flags);
    }
    super::isolation::run(command)
}

pub(super) fn docs() -> Result<u8, String> {
    let mut command = Command::new("cargo");
    command.args(["doc", "--workspace", "--no-deps", "--locked"]);
    command.env("RUSTDOCFLAGS", "-D warnings -D missing_docs");
    super::isolation::run(command)
}
