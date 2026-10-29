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
    let output = toolchain_support::fixture_command(env!("CARGO"), root.path(), None)
        .unwrap()
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

/// Registry fixtures resolve supplied vendored sources without an ambient cache.
#[test]
fn fixture_cargo_resolution_uses_supplied_sources() {
    let root = Scratch::new().unwrap();
    let dependency = root.path().join("vendor/cache-dependency");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "").unwrap();
    fs::write(dependency.join("src/lib.rs"), "").unwrap();
    fs::write(
        dependency.join("Cargo.toml"),
        "[package]\nname = 'cache-dependency'\nversion = '0.0.0'\nedition = '2024'\n",
    )
    .unwrap();
    fs::write(
        dependency.join(".cargo-checksum.json"),
        r#"{"files":{},"package":null}"#,
    )
    .unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = 'cache-fixture'\nversion = '0.0.0'\nedition = '2024'\n\
         [workspace]\n[dependencies]\ncache-dependency = '=0.0.0'\n",
    )
    .unwrap();
    let sources = root.path().join("sources.toml");
    fs::write(
        &sources,
        format!(
            "[source.crates-io]\nreplace-with = 'fixtures'\n[source.fixtures]\n\
             directory = {}\n[net]\noffline = true\n",
            serde_json::to_string(&root.path().join("vendor")).unwrap()
        ),
    )
    .unwrap();
    let output = toolchain_support::fixture_command(env!("CARGO"), root.path(), Some(&sources))
        .unwrap()
        .args(["metadata", "--offline", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(metadata["packages"].as_array().unwrap().len(), 2);
    assert!(!root.path().join("cargo-home/registry").exists());
}
