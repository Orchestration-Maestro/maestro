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
//! the same way because Cargo records that spelling in what it compiles. A recipe that fails in
//! a shard only loses the saving: the tests run the same recipes and report their own failures.
//! The preparation, which builds the graph for every shard, fails instead.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::SystemTime;

use super::normalize::set_tree_time;
use super::{cargo, export, nonempty, prepared_bundle};
use crate::copy_assets::copy_tree;
use crate::test_setup::{install_browser_graph, remove_directory};

/// The checkout directory the graph is built in and the tests are told to use.
pub(crate) const TARGET: &str = "target/browser-author";

/// The bundle directory that holds the staged graph.
pub(crate) const STAGED: &str = "browser-author";

/// The graph's incremental compilation state, which Cargo rewrites on the next build.
const INCREMENTAL: &str = "debug/incremental";

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
    if let Err(error) = build(checkout, &cargo()) {
        eprintln!("warning: {error}; the browser tests report it themselves");
    }
    let mut entry = OsString::from("MAESTRO_BROWSER_TARGET_DIR=");
    entry.push(&target);
    export(&entry)?;
    Ok(ExitCode::SUCCESS)
}

/// Build the browser graph in `target/browser-author` with the Cargo executable `cargo`.
///
/// # Errors
/// Returns the commands that failed, after every command has run.
pub(crate) fn build(checkout: &Path, cargo: &OsStr) -> io::Result<()> {
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
    let mut failed: Vec<String> = Vec::new();
    failed.extend(failure(lint));
    for recipe in RECIPES {
        let mut just = command("just", checkout, &target);
        just.arg(recipe);
        failed.extend(failure(just));
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "the browser graph did not build: {} failed",
            failed.join(", ")
        )))
    }
}

/// Copy the graph built in `target` into `staged`, without the incremental state that no later
/// build reads and that is a seventh of its size.
pub(crate) fn stage(target: &Path, staged: &Path) -> io::Result<()> {
    remove_directory(&target.join(INCREMENTAL))?;
    copy_tree(target, staged)
}

/// Replace `target` with the graph staged in `staged`, and give every file one modification time
/// that is newer than the checkout, so Cargo finds nothing to rebuild.
pub(crate) fn install(staged: &Path, target: &Path) -> io::Result<()> {
    remove_directory(target)?;
    copy_tree(staged, target)?;
    set_tree_time(target, SystemTime::now())
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

/// Run `command`. The command line when it did not succeed.
fn failure(mut command: Command) -> Option<String> {
    if command.status().is_ok_and(|status| status.success()) {
        return None;
    }
    let arguments = command
        .get_args()
        .map(|argument| argument.to_string_lossy());
    Some(
        std::iter::once(command.get_program().to_string_lossy())
            .chain(arguments)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(test)]
mod tests;
