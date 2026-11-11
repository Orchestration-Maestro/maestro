//! Locate the sandbox runtime script that the crate embeds.
//!
//! The repository tooling writes the script under the workspace `target/` directory with
//! `just sandbox-runtime`; it is never committed. A build without it stops here with one
//! message that names the step.

use std::{env, path::PathBuf};

/// The script the tooling writes, relative to the workspace root.
const SCRIPT: &str = "target/sandbox-runtime/maestro-sandbox-runtime.js";

/// Point the crate at the script, or fail the build naming the step that writes it.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let root = env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .and_then(|manifest| manifest.ancestors().nth(2).map(PathBuf::from))
        .unwrap_or_default();
    let script = root.join(SCRIPT);
    println!("cargo::rerun-if-changed={}", script.display());
    if script.is_file() {
        println!(
            "cargo::rustc-env=MAESTRO_SANDBOX_RUNTIME={}",
            script.display()
        );
    } else {
        println!(
            "cargo::error=the sandbox runtime script {} is missing: run `just sandbox-runtime` first",
            script.display()
        );
    }
}
