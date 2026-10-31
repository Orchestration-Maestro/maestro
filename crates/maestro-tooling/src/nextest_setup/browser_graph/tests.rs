//! Unit coverage for staging and installing the browser graph with a real Cargo build.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use maestro_test_directory::Scratch;

use super::{install, set_tree_time, stage};

/// The build script of the workspace's feature unifier, which every native workspace crate builds
/// on, so the fixture's dependency runs the script a real graph runs.
const UNIFIER_BUILD_SCRIPT: &str = include_str!("../../../../maestro-dependencies/build.rs");

/// Write a package that builds on a path dependency with the unifier's build script, so a build
/// compiles two units and runs one script.
fn write_checkout(root: &Path) {
    fs::create_dir_all(root.join("dependency/src")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"graph\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [dependencies]\ndependency = { path = \"dependency\" }\n",
    )
    .unwrap();
    fs::write(
        root.join("dependency/Cargo.toml"),
        "[package]\nname = \"dependency\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(root.join("dependency/build.rs"), UNIFIER_BUILD_SCRIPT).unwrap();
    fs::write(
        root.join("dependency/src/lib.rs"),
        "pub fn one() -> u8 { 1 }\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn two() -> u8 { dependency::one() + 1 }\n",
    )
    .unwrap();
}

/// Build the package at `root` into `target` and return Cargo's report.
fn build(root: &Path, target: &Path) -> String {
    let output = Command::new(env!("CARGO"))
        .current_dir(root)
        .args(["build", "--offline", "--color", "never"])
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_HOME", root.join("cargo-home"))
        .output()
        .unwrap();
    let report = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "{report}");
    report
}

/// A graph installed from a bundle that an earlier job built leaves Cargo nothing to compile in
/// a checkout whose files are all newer than every file the bundle carries.
#[test]
fn installed_graph_leaves_cargo_nothing_to_compile_in_a_newer_checkout() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    write_checkout(root);
    let target = root.join("target/browser-author");
    let staged = root.join("bundle/browser-author");
    assert!(build(root, &target).contains("Compiling dependency"));
    stage(&target, &staged).unwrap();
    // A bundle extracted in a later job keeps the times of the job that built it.
    set_tree_time(&staged, UNIX_EPOCH + Duration::from_secs(1_000_000_000)).unwrap();
    fs::remove_dir_all(&target).unwrap();
    for source in [
        "Cargo.toml",
        "src/lib.rs",
        "dependency/Cargo.toml",
        "dependency/build.rs",
        "dependency/src/lib.rs",
    ] {
        fs::File::open(root.join(source))
            .unwrap()
            .set_modified(SystemTime::now())
            .unwrap();
    }

    install(&staged, &target).unwrap();

    let report = build(root, &target);
    assert!(!report.contains("Compiling"), "{report}");
}
