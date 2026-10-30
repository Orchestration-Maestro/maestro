//! The fixture-sources setup vendors the registry crates the tooling fixtures resolve.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use maestro_test_directory::Scratch;

/// The fixture Cargo: it records its arguments, fails when asked and otherwise creates the
/// vendor directory, its last argument, and prints the source configuration `vendor` prints.
const CARGO: &str = "#!/bin/sh\n\
    echo \"$*\" >> \"$CALLS\"\n\
    if [ -n \"${VENDOR_FAILS-}\" ]; then echo 'vendor exploded' >&2; exit 1; fi\n\
    for last; do :; done\n\
    mkdir -p \"$last\"\n\
    printf '[source.crates-io]\\nreplace-with = \"vendored-sources\"\\n'\n";

/// The fixture checkout's directory that holds the registry closure and the browser fixture.
const FIXTURES: &str = "crates/maestro-tooling/tests/fixtures";

/// Lay out the inputs the setup reads and a Cargo that records its calls.
fn checkout(root: &Path) {
    for (name, text) in [
        ("Cargo.lock", "workspace lock"),
        ("registry_sources/Cargo.toml", "closure manifest"),
        ("registry_sources/src/lib.rs", ""),
        ("browser_author/Cargo.toml", "browser manifest"),
        ("browser_author/Cargo.lock", "browser lock"),
    ] {
        let path = if name == "Cargo.lock" {
            root.join(name)
        } else {
            root.join(FIXTURES).join(name)
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::write(root.join("cargo"), CARGO).unwrap();
    fs::set_permissions(root.join("cargo"), fs::Permissions::from_mode(0o755)).unwrap();
}

/// Run the setup as nextest does, in `root`, and return what it did.
fn setup(root: &Path, fails: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_development"));
    command
        .args(["nextest-setup", "fixture-sources"])
        .current_dir(root)
        .env("CARGO", root.join("cargo"))
        .env("CALLS", root.join("calls"))
        .env("CARGO_MANIFEST_DIR", root.join("crates/maestro-tooling"))
        .env("NEXTEST_ENV", root.join("nextest-env"))
        .env_remove("VENDOR_FAILS");
    if fails {
        command.env("VENDOR_FAILS", "1");
    }
    command.output().unwrap()
}

/// The configuration path the setup printed.
fn printed(output: &Output) -> PathBuf {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    PathBuf::from(String::from_utf8(output.stdout.clone()).unwrap().trim_end())
}

/// The setup vendors the closure into a directory keyed by its inputs, prints the source
/// configuration, and exports its path to nextest.
#[test]
fn fixture_sources_setup_vendors_the_closure_and_exports_the_configuration() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    checkout(root);

    let config = printed(&setup(root, false));

    let directory = config.parent().unwrap();
    assert_eq!(
        directory.parent().unwrap(),
        root.join("target/fixture-sources")
    );
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n[net]\noffline = true\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("nextest-env")).unwrap(),
        format!("MAESTRO_TEST_CARGO_SOURCES={}\n", config.display())
    );
    assert_eq!(
        fs::read_to_string(directory.join("closure/Cargo.lock")).unwrap(),
        "workspace lock"
    );
    assert_eq!(
        fs::read_to_string(directory.join("closure/Cargo.toml")).unwrap(),
        "closure manifest"
    );
    assert_eq!(
        fs::read_to_string(root.join("calls")).unwrap(),
        format!(
            "vendor --offline --manifest-path {0}/closure/Cargo.toml --sync {1}/{FIXTURES}/browser_author/Cargo.toml {0}/vendor\n",
            directory.display(),
            root.display()
        )
    );
}

/// A directory keyed by unchanged inputs is reused, and a changed lock selects another.
#[test]
fn fixture_sources_setup_reuses_a_directory_while_the_inputs_stay_the_same() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    checkout(root);
    let first = printed(&setup(root, false));

    assert_eq!(printed(&setup(root, false)), first);
    assert_eq!(
        fs::read_to_string(root.join("calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );

    fs::write(root.join("Cargo.lock"), "changed lock").unwrap();
    let second = printed(&setup(root, false));
    assert_ne!(second, first);
    assert_eq!(
        fs::read_to_string(root.join("calls"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

/// A failed vendor run shows its log, fails the setup and leaves no configuration to reuse.
#[test]
fn fixture_sources_setup_reports_a_failed_vendor_run() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    checkout(root);

    let output = setup(root, true);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("vendor exploded"));
    let keyed = fs::read_dir(root.join("target/fixture-sources"))
        .unwrap()
        .next();
    assert!(!keyed.unwrap().unwrap().path().join("config.toml").exists());
    assert!(!root.join("nextest-env").exists());
}
