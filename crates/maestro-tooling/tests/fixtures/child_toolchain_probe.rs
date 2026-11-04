//! Instrumented parent of a standalone WebAssembly build.
use maestro_test_process::toolchain as toolchain_support;

use std::env;
use std::process::ExitCode;

/// Build the child while retaining every inherited parent variable.
fn main() -> ExitCode {
    let Some(cargo) = env::var_os("MAESTRO_PROBE_CARGO") else {
        eprintln!("the probe needs MAESTRO_PROBE_CARGO");
        return ExitCode::FAILURE;
    };
    let before: Vec<_> = env::vars_os().collect();
    let Ok(output) = toolchain_support::command(cargo)
        .args(["build", "--locked", "--target", "wasm32-unknown-unknown"])
        .output()
    else {
        eprintln!("the probe could not run the child build");
        return ExitCode::FAILURE;
    };
    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return ExitCode::FAILURE;
    }
    if env::vars_os().collect::<Vec<_>>() != before {
        eprintln!("parent environment changed");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
