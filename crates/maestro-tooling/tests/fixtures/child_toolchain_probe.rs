//! Instrumented parent of a standalone WebAssembly build.
use maestro_test_process::toolchain as toolchain_support;

use std::env;

/// Build the child while retaining every inherited parent variable.
fn main() {
    let before: Vec<_> = env::vars_os().collect();
    let output = toolchain_support::command(env::var_os("MAESTRO_PROBE_CARGO").unwrap())
        .args(["build", "--locked", "--target", "wasm32-unknown-unknown"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(env::vars_os().collect::<Vec<_>>() == before, "parent environment changed");
}
