//! Prepared component coverage and reuse at the shared Cargo boundary.

use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, selection};
use std::{collections::HashMap, fs, path::Path};

#[path = "../../maestro-extensions-wasm/src/tests/build/cargo.rs"]
mod component_build;

#[test]
fn component_consumers_have_prepared_examples() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut prepared = HashMap::new();
    maestro_tooling::example_files::prepare(
        &[
            workspace.join("crates/maestro-extensions-wasm/examples"),
            workspace.join("examples/extensions"),
        ],
        |name, variable| {
            prepared.insert(name.to_owned(), variable.to_owned());
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

/// Checks the literal example operands accepted by the shared test builders, and that no
/// caller reads the example's setup variable, which the builder already looks up.
fn assert_prepared_consumers(file: &Path, prepared: &HashMap<String, String>) {
    let source = fs::read_to_string(file).unwrap();
    for call in [
        "example_component(",
        "Harness::example(",
        "build::component(",
        "\"--example\",",
    ] {
        for (offset, _) in source.match_indices(call) {
            let arguments = source[offset + call.len()..].trim_start();
            let Some(arguments) = arguments.strip_prefix('"') else {
                continue;
            };
            let name = arguments.split('"').next().unwrap();
            let variable = prepared
                .get(name)
                .unwrap_or_else(|| panic!("setup does not prepare {name}: {}", file.display()));
            assert!(
                !source.contains(&format!("\"{variable}\"")),
                "{} looks up {variable} beside the builder for {name}",
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

/// Cargo reports each example's component from the one build that compiles them all.
#[cfg(unix)]
#[test]
fn component_build_compiles_unprepared_examples_in_one_cargo_run() {
    if !selection().is_child() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new().unwrap();
        let directory = scratch.path().join("crates/author");
        fs::create_dir_all(directory.join("examples")).unwrap();
        fs::create_dir_all(scratch.path().join("examples/extensions")).unwrap();
        let calls = scratch.path().join("calls");
        let cargo = scratch.path().join("cargo");
        fs::write(
            &cargo,
            r#"#!/bin/sh
echo "$*" >> "$CARGO_CALLS"
for name in alpha beta; do
    printf '{"reason":"compiler-artifact","target":{"name":"%s"},"filenames":["/built/%s.wasm"]}\n' "$name" "$name"
done
"#,
        )
        .unwrap();
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
        let output =
            TestProcess::current("component_build_compiles_unprepared_examples_in_one_cargo_run")
                .unwrap()
                .case("single")
                .nocapture()
                .env("CARGO", cargo)
                .env("CARGO_CALLS", &calls)
                .env("MAESTRO_COMPONENT_FIXTURE_ROOT", directory)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let calls = fs::read_to_string(calls).unwrap();
        assert_eq!(calls.lines().count(), 1, "{calls}");
        assert!(calls.contains("--example alpha --example beta"), "{calls}");
        return;
    }
    let directory = std::env::var_os("MAESTRO_COMPONENT_FIXTURE_ROOT").unwrap();
    let components =
        component_build::example_components(Path::new(&directory), &["alpha", "beta"]).unwrap();
    assert_eq!(
        components,
        [
            std::path::PathBuf::from("/built/alpha.wasm"),
            std::path::PathBuf::from("/built/beta.wasm")
        ]
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

/// Runs the cache normalization from `root`, where the lock file and sources are found,
/// over its `target/debug` profile directory. An archived run sets the archive signal that
/// shared CI gives nextest; any other run clears it, whatever this test run inherited.
#[cfg(unix)]
fn normalize_run(root: &Path, archived: bool) {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_development"));
    command
        .args(["nextest-setup", "normalize-component-cache"])
        .arg(root.join("target/debug"))
        .current_dir(root)
        .env_remove("TEST_ARCHIVE");
    if archived {
        command.env("TEST_ARCHIVE", "1");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Runs the normalization as a local run does, where Cargo may rebuild a binary later.
#[cfg(unix)]
fn normalize(root: &Path) {
    normalize_run(root, false);
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
    normalize(scratch.path());
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
    normalize(scratch.path());
    let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
    assert_eq!(modified(&older), at(50));
    assert_eq!(modified(&newer), at(60));
}

/// Nextest never runs Cargo on an archive, so a library output newer than a restored binary
/// outdates nothing: every executable shares the newest time, whether or not Cargo left
/// dependency information beside it.
#[cfg(unix)]
#[test]
fn cache_normalization_advances_every_binary_of_an_archived_run() {
    let scratch = Scratch::new().unwrap();
    let deps = scratch.path().join("target/debug/deps");
    lock(scratch.path(), &[("alpha", &[]), ("beta", &["alpha"])]);
    output(&deps, "libalpha-1.rlib", false, 300);
    let bare = output(&deps, "bare-1", true, 50);
    let beta = binary(&deps, "it-beta", "crates/beta/tests/it.rs", 100);
    let newest = output(&deps, "fresh-1", true, 400);
    normalize_run(scratch.path(), true);
    let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
    assert_eq!(modified(&bare), at(400));
    assert_eq!(modified(&beta), at(400));
    assert_eq!(modified(&newest), at(400));
}

/// Writes a lock file naming each workspace crate with the crates it builds on, and one
/// registry crate that depends on nothing.
#[cfg(unix)]
fn lock(root: &Path, crates: &[(&str, &[&str])]) {
    let mut packages: Vec<String> = crates
        .iter()
        .map(|(name, dependencies)| {
            let listed = dependencies
                .iter()
                .map(|dependency| format!("\"{dependency}\""))
                .collect::<Vec<_>>()
                .join(",\n ");
            let table = if listed.is_empty() {
                String::new()
            } else {
                format!("dependencies = [\n {listed},\n]\n")
            };
            format!("[[package]]\nname = \"{name}\"\nversion = \"0.1.0\"\n{table}")
        })
        .collect();
    packages.push(
        "[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n\
         source = \"registry+https://github.com/rust-lang/crates.io-index\"\n"
            .to_owned(),
    );
    fs::write(
        root.join("Cargo.lock"),
        format!("version = 4\n\n{}", packages.join("\n")),
    )
    .unwrap();
}

/// Writes an executable with the dependency information Cargo leaves beside it.
#[cfg(unix)]
fn binary(deps: &Path, name: &str, source: &str, seconds: u64) -> std::path::PathBuf {
    let path = output(deps, name, true, seconds);
    let information = format!("{}: {source} {source}.helper\n", path.display());
    fs::write(deps.join(format!("{name}.d")), information).unwrap();
    path
}

/// A library that a binary does not build on cannot make it outdated, so a rebuilt unrelated
/// crate does not keep the setup example and the test binaries from sharing one time.
#[cfg(unix)]
#[test]
fn cache_normalization_ignores_libraries_a_binary_does_not_build_on() {
    let scratch = Scratch::new().unwrap();
    let deps = scratch.path().join("target/debug/deps");
    lock(
        scratch.path(),
        &[
            ("alpha", &[]),
            ("beta", &["alpha", "serde"]),
            ("gamma", &[]),
            ("delta", &["alpha"]),
        ],
    );
    output(&deps, "libalpha-1.rlib", false, 100);
    output(&deps, "libgamma-1.rlib", false, 300);
    output(&deps, "libserde-1.rlib", false, 500);
    let beta = binary(&deps, "it-beta", "crates/beta/tests/it.rs", 150);
    let gamma = binary(&deps, "it-gamma", "crates/gamma/tests/it.rs", 400);
    let delta = binary(&deps, "it-delta", "crates/delta/tests/it.rs", 50);
    normalize(scratch.path());
    let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
    assert_eq!(modified(&beta), at(400));
    assert_eq!(modified(&gamma), at(400));
    assert_eq!(modified(&delta), at(50));
}

/// A library's own test binary is built from the crate, not on its output, so only the
/// crate's other binaries are outdated by a newer library.
#[cfg(unix)]
#[test]
fn cache_normalization_treats_a_library_test_as_independent_of_its_own_library() {
    let scratch = Scratch::new().unwrap();
    let deps = scratch.path().join("target/debug/deps");
    lock(scratch.path(), &[("beta", &[]), ("gamma", &[])]);
    output(&deps, "libbeta-1.rlib", false, 160);
    let unit = binary(&deps, "beta-1", "crates/beta/src/lib.rs", 150);
    let integration = binary(&deps, "it-beta", "crates/beta/tests/it.rs", 140);
    let anchor = binary(&deps, "it-gamma", "crates/gamma/tests/it.rs", 400);
    normalize(scratch.path());
    let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
    assert_eq!(modified(&unit), at(400));
    assert_eq!(modified(&integration), at(140));
    assert_eq!(modified(&anchor), at(400));
}
