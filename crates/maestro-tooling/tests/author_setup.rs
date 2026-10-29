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
    for archived in [false, true] {
        assert_success(&export(root, &moved, archived));
    }
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

/// Missing, edited or incomplete reusable outputs fail without starting Cargo.
#[test]
fn archived_author_setup_rejects_missing_and_stale_outputs() {
    for change in [
        "directory",
        "component",
        "source",
        "added",
        "removed",
        "lock",
        "corrupt",
    ] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        assert_success(&prepare(root));
        let prepared = root.join("target/test-setup");
        match change {
            "directory" => fs::rename(&prepared, root.join("unused")).unwrap(),
            "component" => fs::remove_file(prepared.join("MAESTRO_AUTHOR_COMPONENT.wasm")).unwrap(),
            "source" => fs::write(root.join("crates/example/src/lib.rs"), "edited source").unwrap(),
            "added" => fs::write(root.join("crates/example/src/new.rs"), "new source").unwrap(),
            "removed" => fs::remove_file(root.join("crates/example/src/lib.rs")).unwrap(),
            "lock" => fs::write(root.join("Cargo.lock"), "new dependencies").unwrap(),
            "corrupt" => {
                fs::write(prepared.join("MAESTRO_AUTHOR_COMPONENT.wasm"), "broken").unwrap();
            }
            _ => unreachable!(),
        }
        let output = export(root, &prepared, true);
        assert!(!output.status.success(), "accepted {change}");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains(prepared.to_str().unwrap()),
            "{diagnostic}"
        );
        assert!(!root.join("nextest-env").exists());
        assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
    }
}

/// Local and warm runs without a prepared bundle keep automatic preparation.
#[test]
fn author_setup_without_prepared_outputs_builds_locally() {
    for mode in ["local", "warm"] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        let profile = root.join("target/debug");
        fs::create_dir_all(profile.join("deps")).unwrap();
        let mut command = Command::new("sh");
        command
            .arg(script("prepare-author-component.sh"))
            .current_dir(root)
            .env("CARGO", root.join("cargo"))
            .env("NEXTEST_ENV", root.join("nextest-env"))
            .env("NEXTEST_LD_LIBRARY_PATH", &profile)
            .env("NEXTEST_DYLD_FALLBACK_LIBRARY_PATH", &profile)
            .env_remove("TEST_ARCHIVE")
            .env_remove("TEST_PREPARE_OUTPUT");
        if mode == "warm" {
            command.env("TEST_PREPARE_OUTPUT", root.join("target/test-setup"));
        }
        assert_success(&command.output().unwrap());
        assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
        assert!(root.join("nextest-env").is_file());
        assert!(!root.join("target/test-setup").exists());
    }
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
fn export(root: &Path, prepared: &Path, archived: bool) -> Output {
    let mut command = Command::new("sh");
    if archived {
        command.env("TEST_ARCHIVE", "1");
    } else {
        command.env_remove("TEST_ARCHIVE");
    }
    command
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
