//! A guest-exported resource cannot be registered through an import of the same resource.

use std::process::Command;

/// Checks that registering a guest-exported handler through the import fails to compile.
#[test]
fn exported_resource_cannot_be_registered_through_an_import() -> std::io::Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args([
            "check",
            "--color",
            "never",
            "--manifest-path",
            "../counterexample/Cargo.toml",
        ])
        .env("CARGO_TARGET_DIR", "../target/counterexample")
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "the counterexample must not compile"
    );
    assert_eq!(
        stderr.matches("error[").count(),
        1,
        "only the registration call fails:\n{stderr}"
    );
    assert!(
        stderr.contains("error[E0308]: mismatched types"),
        "{stderr}"
    );
    assert!(
        stderr.contains("expected `maestro::counterexample::guest::Handler`, found `Handler`"),
        "{stderr}"
    );
    Ok(())
}
