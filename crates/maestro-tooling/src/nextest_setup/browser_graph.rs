//! Builds the browser graph the browser author tests share, once per run.
//!
//! A shard starts with an empty target directory and no compile cache. Without this setup the
//! first browser test of the shard compiles the native bootstrap and the WebAssembly fixture
//! itself, while a thousand other tests compete for the same cores, and the slowest of them
//! reaches the hang limit. The test preparation builds the graph once and stages it in the
//! bundle; a shard installs it and gives it one modification time newer than its checkout, so
//! the recipes find nothing to rebuild. A run without a bundle builds the graph itself. The
//! recipes run in this checkout with the runner's instrumentation removed, as the tests remove
//! it from their own children, and in the target directory the tests are told to use, spelled
//! the same way because Cargo records that spelling in what it compiles. A recipe that fails
//! here only loses the saving: the tests run the same recipes and report their own failures.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use super::{cargo, export, nonempty, prepared_bundle};
use crate::test_setup::install_browser_graph;

/// The checkout directory the graph is built in and the tests are told to use.
pub(crate) const TARGET: &str = "target/browser-author";

/// The bundle directory that holds the staged graph.
pub(crate) const STAGED: &str = "browser-author";

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

/// Install the bundle's graph when a run has one, then build whatever is still missing and
/// export where the graph is.
///
/// # Errors
/// Returns a stale bundle, a failed install or a failed export. A failed build only prints a
/// warning.
pub(super) fn run(checkout: &Path) -> io::Result<ExitCode> {
    let target = checkout.join(TARGET);
    if let Some(bundle) = prepared_bundle(checkout) {
        install_browser_graph(checkout, &bundle, &target)?;
    }
    build(checkout, &cargo());
    let mut entry = OsString::from("MAESTRO_BROWSER_TARGET_DIR=");
    entry.push(&target);
    export(&entry)?;
    Ok(ExitCode::SUCCESS)
}

/// Build the browser graph in `target/browser-author` with the Cargo executable `cargo`.
/// Whether every command succeeded.
pub(crate) fn build(checkout: &Path, cargo: &OsStr) -> bool {
    let target = checkout.join(TARGET);
    let mut lint = command(cargo, checkout, &target);
    // The library lint of `browser-check`, without its formatting repair, which would rewrite
    // sources.
    lint.args([
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
    let mut built = attempt(lint);
    for recipe in RECIPES {
        let mut just = command("just", checkout, &target);
        just.arg(recipe);
        built &= attempt(just);
    }
    built
}

/// Copy the graph built in `target` into `staged`.
pub(crate) fn stage(_target: &Path, staged: &Path) -> io::Result<()> {
    std::fs::create_dir_all(staged)
}

/// Replace `target` with the graph staged in `staged`.
pub(crate) fn install(_staged: &Path, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(target)
}

/// A command for `program` in the checkout, without the runner's instrumentation and with the
/// compiler wrapper that preceded coverage restored.
fn command(program: impl AsRef<OsStr>, checkout: &Path, target: &Path) -> Command {
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

/// Run `command`, and warn instead of failing when it does not succeed. Whether it did.
fn attempt(mut command: Command) -> bool {
    let succeeded = command.status().is_ok_and(|status| status.success());
    if !succeeded {
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
    succeeded
}

#[cfg(test)]
mod tests;
