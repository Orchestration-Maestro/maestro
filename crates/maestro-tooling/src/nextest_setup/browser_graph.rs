//! Builds the browser graph the browser author tests share, once before they run.
//!
//! A shard starts with an empty target directory and no compile cache. Without this setup the
//! first browser test of the shard compiles the native bootstrap and the WebAssembly fixture
//! itself, while a thousand other tests compete for the same cores, and the slowest of them
//! reaches the hang limit. The recipes run in this checkout with the runner's instrumentation
//! removed, as the tests remove it from their own children, and in the target directory the
//! tests are told to use, spelled the same way because Cargo records that spelling in what it
//! compiles. A recipe that fails here only loses the saving: the tests run the same recipes
//! and report their own failures.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use super::{export, nonempty};

/// Variables of the coverage runner that change what Cargo compiles.
const INSTRUMENTATION: [&str; 9] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "LLVM_PROFILE_FILE",
    "CARGO_LLVM_COV",
    "CARGO_LLVM_COV_TARGET_DIR",
    "CARGO_LLVM_COV_BUILD_DIR",
    "CARGO_LLVM_COV_SHOW_ENV",
    "__CARGO_LLVM_COV_RUSTC_WRAPPER",
    "__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING",
];

/// The recipes that build the shared graph, after the library lint.
const RECIPES: [&str; 3] = [
    "browser-example-check",
    "browser-library-build",
    "browser-debug-build",
];

/// Build the browser graph in `target/browser-author` and export where it is.
///
/// # Errors
/// Returns a failed export. A failed build only prints a warning.
pub(super) fn run(checkout: &Path) -> io::Result<ExitCode> {
    let target = checkout.join("target/browser-author");
    let mut cargo = command("cargo", checkout, &target);
    // The library lint of `browser-check`, without its formatting repair, which would rewrite
    // sources.
    cargo.args([
        "clippy",
        "--no-deps",
        "-p",
        "maestro-web",
        "--target",
        "wasm32-unknown-unknown",
        "--locked",
        "--",
        "-D",
        "warnings",
    ]);
    attempt(cargo);
    for recipe in RECIPES {
        let mut just = command("just", checkout, &target);
        just.arg(recipe);
        attempt(just);
    }
    let mut entry = OsString::from("MAESTRO_BROWSER_TARGET_DIR=");
    entry.push(&target);
    export(&entry)?;
    Ok(ExitCode::SUCCESS)
}

/// A command for `program` in the checkout, without the runner's instrumentation and with the
/// compiler wrapper that preceded coverage restored.
fn command(program: &str, checkout: &Path, target: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(checkout);
    if nonempty("__CARGO_LLVM_COV_RUSTC_WRAPPER").is_some() {
        match nonempty("__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING") {
            Some(wrapper) => command.env("RUSTC_WRAPPER", wrapper),
            None => command.env_remove("RUSTC_WRAPPER"),
        };
    }
    for name in INSTRUMENTATION {
        command.env_remove(name);
    }
    command
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_BUILD_JOBS", "3")
        .env("TRUNK_SKIP_VERSION_CHECK", "true");
    command
}

/// Run `command`, and warn instead of failing when it does not succeed.
fn attempt(mut command: Command) {
    if !command.status().is_ok_and(|status| status.success()) {
        let arguments: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect();
        eprintln!(
            "warning: {} {} failed; the browser tests report it themselves",
            command.get_program().to_string_lossy(),
            arguments.join(" ")
        );
    }
}
