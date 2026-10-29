//! Cargo fixture cache isolation through the shared command builder.

use crate::toolchain_support;
use maestro_test_directory::Scratch;
use std::fs;

/// Dependency-free fixture resolution creates only its private cache lock.
#[test]
fn fixture_cargo_resolution_uses_private_home() {
    let root = Scratch::new().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = 'cache-fixture'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    let output = toolchain_support::fixture_command(env!("CARGO"), root.path())
        .args(["metadata", "--offline", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        root.path().join("cargo-home/.package-cache").is_file(),
        "fixture resolution must create its private package-cache lock"
    );
}
