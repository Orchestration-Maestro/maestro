//! Prepared component coverage and reuse at the shared Cargo boundary.

use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, selection};
use std::{collections::HashSet, fs, path::Path};

#[path = "../../maestro-extensions-wasm/src/tests/build/cargo.rs"]
mod component_build;

#[test]
fn component_consumers_have_prepared_examples() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut prepared = HashSet::new();
    maestro_tooling::example_files::prepare(
        &[
            workspace.join("crates/maestro-extensions-wasm/examples"),
            workspace.join("examples/extensions"),
        ],
        |name, _| {
            prepared.insert(name.to_owned());
            Ok(name.into())
        },
    )
    .unwrap();
    for folder in [
        "crates/maestro-extensions-wasm/src/tests",
        "crates/maestro-extensions-wasmtime/tests",
    ] {
        for file in maestro_tooling::example_files::collect(&workspace.join(folder)).unwrap() {
            if file.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            assert_prepared_consumers(&file, &prepared);
        }
    }
}

/// Checks the literal example operands accepted by the shared test builders.
fn assert_prepared_consumers(file: &Path, prepared: &HashSet<String>) {
    let source = fs::read_to_string(file).unwrap();
    for call in ["example_component(", "Harness::example(", "\"--example\","] {
        for (offset, _) in source.match_indices(call) {
            let arguments = source[offset + call.len()..].trim_start();
            let Some(arguments) = arguments.strip_prefix('"') else {
                continue;
            };
            let name = arguments.split('"').next().unwrap();
            assert!(
                prepared.contains(name),
                "setup does not prepare {name}: {}",
                file.display()
            );
        }
    }
}

#[test]
fn component_build_reuses_the_prepared_example() {
    if !selection().is_child() {
        let scratch = Scratch::new().unwrap();
        let directory = scratch.path().join("crates/author");
        fs::create_dir_all(directory.join("examples")).unwrap();
        fs::create_dir_all(scratch.path().join("examples/extensions")).unwrap();
        fs::write(
            directory.join("examples/fixture.rs"),
            "// maestro-component: MAESTRO_PREPARED_FIXTURE_COMPONENT\n",
        )
        .unwrap();
        let component = scratch.path().join("fixture.wasm");
        fs::write(&component, b"\0asm\x0d\0\x01\0").unwrap();
        let output = TestProcess::current("component_build_reuses_the_prepared_example")
            .unwrap()
            .case("prepared")
            .nocapture()
            .env("MAESTRO_PREPARED_FIXTURE_COMPONENT", component)
            .env("MAESTRO_COMPONENT_FIXTURE_ROOT", directory)
            .env("CARGO", scratch.path().join("missing-cargo"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let directory = std::env::var_os("MAESTRO_COMPONENT_FIXTURE_ROOT").unwrap();
    let component =
        component_build::component(Path::new(&directory), &["--example", "fixture"], "fixture")
            .expect("prepared example must not invoke Cargo");
    assert_eq!(
        component,
        std::path::PathBuf::from(std::env::var_os("MAESTRO_PREPARED_FIXTURE_COMPONENT").unwrap())
    );
}

/// Seconds after the epoch, as the timestamps these fixtures assign.
#[cfg(unix)]
fn at(seconds: u64) -> std::time::SystemTime {
    std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)
}

/// Writes an executable or a library output into `deps` with a fixed modification time.
#[cfg(unix)]
fn output(deps: &Path, name: &str, executable: bool, seconds: u64) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = deps.join(name);
    fs::create_dir_all(deps).unwrap();
    fs::write(&path, b"#!/bin/sh\n").unwrap();
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    fs::File::open(&path)
        .unwrap()
        .set_modified(at(seconds))
        .unwrap();
    path
}

/// Runs the cache normalization over a fake profile directory.
#[cfg(unix)]
fn normalize(profile: &Path) {
    let output = std::process::Command::new("sh")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.config/normalize-component-cache.sh"),
        )
        .arg(profile)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Cargo rebuilds a test binary only while it looks older than a dependency's output, so
/// normalization may not advance a binary that a rebuilt library has outdated.
#[cfg(unix)]
#[test]
fn cache_normalization_leaves_stale_test_binaries_alone() {
    let scratch = Scratch::new().unwrap();
    let profile = scratch.path().join("target/debug");
    let deps = profile.join("deps");
    let stale = output(&deps, "stale-1", true, 50);
    output(&deps, "libdependency-1.rlib", false, 100);
    let fresh = output(&deps, "fresh-1", true, 150);
    let newest = output(&deps, "fresh-2", true, 200);
    normalize(&profile);
    let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
    assert_eq!(modified(&stale), at(50));
    assert_eq!(modified(&fresh), at(200));
    assert_eq!(modified(&newest), at(200));
}

/// A run whose binaries are all outdated has nothing to advance, and setup still proceeds.
#[cfg(unix)]
#[test]
fn cache_normalization_accepts_a_profile_with_only_stale_binaries() {
    let scratch = Scratch::new().unwrap();
    let profile = scratch.path().join("target/debug");
    let deps = profile.join("deps");
    let older = output(&deps, "older-1", true, 50);
    let newer = output(&deps, "newer-1", true, 60);
    output(&deps, "libdependency-1.rlib", false, 100);
    normalize(&profile);
    let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
    assert_eq!(modified(&older), at(50));
    assert_eq!(modified(&newer), at(60));
}
