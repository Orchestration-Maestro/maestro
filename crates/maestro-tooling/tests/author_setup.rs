//! Standalone component preparation and archived nextest setup.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use maestro_test_directory::Scratch;

/// Reusable outputs survive a different checkout and never start a shard build.
#[test]
fn archived_author_setup_reuses_relocated_components() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    let prepared = prepare(root);
    assert_success(&prepared);
    assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
    let moved = root.join("relocated setup");
    fs::rename(root.join("target/test-setup"), &moved).unwrap();
    fs::remove_file(root.join("component.wasm")).unwrap();
    let output = export(root, &moved);
    assert_success(&output);
    let environment = fs::read_to_string(root.join("nextest-env")).unwrap();
    for variable in ["MAESTRO_AUTHOR_COMPONENT", "MAESTRO_CONTEXT_COMPONENT"] {
        let path = environment
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{variable}=")))
            .unwrap();
        assert!(Path::new(path).starts_with(&moved));
        assert_eq!(fs::read(path).unwrap(), b"component bytes");
    }
    assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
}

/// Give the standalone command a controlled Cargo boundary and source checkout.
fn fixture(root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(root.join("crates/example/src/lib.rs"), "original source").unwrap();
    fs::write(root.join("Cargo.lock"), "locked dependencies").unwrap();
    fs::write(root.join("component.wasm"), "component bytes").unwrap();
    fs::write(
        root.join("cargo"),
        "#!/bin/sh\nset -eu\necho build >> builds\nprintf 'MAESTRO_AUTHOR_COMPONENT=%s/component.wasm\\nMAESTRO_CONTEXT_COMPONENT=%s/component.wasm\\n' \"$PWD\" \"$PWD\" > \"$NEXTEST_ENV\"\n",
    )
    .unwrap();
    fs::set_permissions(root.join("cargo"), fs::Permissions::from_mode(0o755)).unwrap();
    for args in [["init", "-q"].as_slice(), ["add", "."].as_slice()] {
        assert_success(
            &Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap(),
        );
    }
}

/// Invoke the standalone preparation command, not a native test executable.
fn prepare(root: &Path) -> Output {
    Command::new("python3")
        .arg(script("prepare-test-setup.py"))
        .arg("prepare")
        .current_dir(root)
        .env("CARGO", root.join("cargo"))
        .env("TEST_PREPARE_OUTPUT", root.join("target/test-setup"))
        .output()
        .unwrap()
}

/// Run the real nextest setup entry point with archived-shard environment.
fn export(root: &Path, prepared: &Path) -> Output {
    Command::new("sh")
        .arg(script("prepare-author-component.sh"))
        .current_dir(root)
        .env("CARGO", root.join("cargo"))
        .env("TEST_PREPARE_OUTPUT", prepared)
        .env("NEXTEST_ENV", root.join("nextest-env"))
        .output()
        .unwrap()
}

/// Locate a repository setup script without embedding its contents.
fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.config")
        .join(name)
}

/// Display both output streams when a fixture command fails.
fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
