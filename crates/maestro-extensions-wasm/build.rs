//! Discover example test modules without a handwritten registration list.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "build/test_modules.rs"]
/// Test module discovery shared with the repository command checks.
mod test_modules;

/// Write the discovered declarations into Cargo's private output directory.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").ok_or("missing manifest directory")?,
    );
    let directory = root.join("src/tests");
    println!("cargo:rerun-if-changed={}", directory.display());
    let output = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").ok_or("missing build output directory")?,
    );
    std::fs::write(
        output.join("example_tests.rs"),
        test_modules::generate(&directory)?,
    )?;
    Ok(())
}
