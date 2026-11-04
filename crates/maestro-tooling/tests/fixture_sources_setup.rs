//! The fixture-sources setup vendors the registry crates the tooling fixtures resolve.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use maestro_test_directory::Scratch;

use crate::recipe_support::{installed, write_input};

/// The fixture checkout's directory that holds the registry closure and the browser fixture.
const FIXTURES: &str = "crates/maestro-tooling/tests/fixtures";

/// Lay out the inputs the setup reads.
fn checkout(root: &Path) {
    for (name, text) in [
        ("Cargo.lock", "workspace lock"),
        ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.0.0\"\n"),
        ("registry_sources/Cargo.toml", "closure manifest"),
        ("registry_sources/src/lib.rs", ""),
        ("browser_author/Cargo.toml", "browser manifest"),
        ("browser_author/Cargo.lock", "browser lock"),
    ] {
        let path = if name == "Cargo.lock" || name == "rust-toolchain.toml" {
            root.join(name)
        } else {
            root.join(FIXTURES).join(name)
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

/// The operation the fixture Cargo fails, if any.
#[derive(Clone, Copy)]
enum Failure {
    /// Both operations succeed.
    None,
    /// `cargo vendor` fails.
    Vendor,
    /// `cargo build` fails.
    Build,
}

/// Install the fixture Cargo, which records its calls and fails as asked, and configure the setup
/// command as nextest does, with isolated Cargo and export paths.
fn setup(root: &Path, failure: Failure) -> Command {
    let fails = match failure {
        Failure::None => None,
        Failure::Vendor => Some("vendor"),
        Failure::Build => Some("build"),
    };
    let record = serde_json::json!({
        "scenario": "sources",
        "calls": root.join("calls"),
        "builds": root.join("builds"),
        "fails": fails,
    });
    installed(root, "cargo", &format!("tooling-sources\n{record}\n"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_development"));
    command
        .args(["nextest-setup", "fixture-sources"])
        .current_dir(root)
        .env("CARGO", root.join("cargo"))
        .env("NEXTEST_ENV", root.join("nextest-env"));
    command
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

    let config = printed(&setup(root, Failure::None).output().unwrap());

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

/// Unchanged inputs reuse sources; changed locks or browser dependencies select fresh sources.
#[test]
fn fixture_sources_setup_reuses_a_directory_while_the_inputs_stay_the_same() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    checkout(root);
    let first = printed(&setup(root, Failure::None).output().unwrap());

    assert_eq!(
        printed(&setup(root, Failure::None).output().unwrap()),
        first
    );
    assert_eq!(
        fs::read_to_string(root.join("calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );

    let mut previous = first;
    for (index, input) in [
        "Cargo.lock".to_owned(),
        format!("{FIXTURES}/browser_author/Cargo.toml"),
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(root.join(&input), "changed dependency input").unwrap();
        let current = printed(&setup(root, Failure::None).output().unwrap());
        assert_ne!(current, previous, "stale sources after editing {input}");
        assert_eq!(
            fs::read_to_string(root.join("calls"))
                .unwrap()
                .lines()
                .count(),
            index + 2
        );
        previous = current;
    }
}

/// A failed vendor run shows its log, fails the setup and leaves no configuration to reuse.
#[test]
fn fixture_sources_setup_reports_a_failed_vendor_run() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    checkout(root);

    let output = setup(root, Failure::Vendor).output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("vendor exploded"));
    let keyed = fs::read_dir(root.join("target/fixture-sources"))
        .unwrap()
        .next();
    assert!(!keyed.unwrap().unwrap().path().join("config.toml").exists());
    assert!(!root.join("nextest-env").exists());
}

/// Dependency declarations, locks and the toolchain refresh the complete workspace source
/// configuration, which also keys the compiled seed.
#[test]
fn workspace_sources_refresh_when_dependency_inputs_change() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    checkout(root);
    write_input(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write_input(
        root,
        "crates/example/Cargo.toml",
        "[dependencies]\nserde = \"=1.0.229\"\n",
    );
    let prepare = || {
        printed(
            &setup(root, Failure::None)
                .arg("workspace")
                .output()
                .unwrap(),
        )
    };
    let mut previous = prepare();
    assert_eq!(prepare(), previous);
    for (index, input) in [
        "Cargo.toml".to_owned(),
        "crates/example/Cargo.toml".to_owned(),
        "Cargo.lock".to_owned(),
        "rust-toolchain.toml".to_owned(),
        format!("{FIXTURES}/browser_author/Cargo.toml"),
        format!("{FIXTURES}/browser_author/Cargo.lock"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = root.join(&input);
        let text = fs::read_to_string(&path).unwrap();
        fs::write(
            path,
            format!("{text}\n[dependencies.added]\nversion = \"=1.0.0\"\n"),
        )
        .unwrap();
        let current = prepare();
        assert_ne!(current, previous, "stale sources after editing {input}");
        assert_eq!(prepare(), current);
        assert_eq!(
            fs::read_to_string(root.join("calls"))
                .unwrap()
                .lines()
                .count(),
            index + 2
        );
        previous = current;
    }
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert!(
        calls
            .lines()
            .all(|call| call.starts_with("vendor --offline --locked --manifest-path "))
    );
    assert!(calls.contains(&format!(
        "--manifest-path {}/Cargo.toml --sync",
        root.display()
    )));
    assert!(
        fs::read_to_string(root.join("nextest-env"))
            .unwrap()
            .contains("MAESTRO_TEST_WORKSPACE_CARGO_SOURCES=")
    );
}

/// Rust-only changes do not cause another vendoring operation.
#[test]
fn workspace_sources_reuse_after_rust_only_edits() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    checkout(root);
    write_input(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write_input(
        root,
        "crates/example/Cargo.toml",
        "[package]\nname = \"example\"\n",
    );
    write_input(
        root,
        "crates/example/src/lib.rs",
        "pub fn value() -> u8 { 1 }\n",
    );
    let first = printed(
        &setup(root, Failure::None)
            .arg("workspace")
            .output()
            .unwrap(),
    );

    fs::write(
        root.join("crates/example/src/lib.rs"),
        "pub fn value() -> u8 { 2 }\n",
    )
    .unwrap();

    assert_eq!(
        printed(
            &setup(root, Failure::Vendor)
                .arg("workspace")
                .output()
                .unwrap()
        ),
        first
    );
    assert_eq!(
        fs::read_to_string(root.join("calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

/// A workspace checkout with the member `example`, which the fixture Cargo builds units for.
fn workspace(root: &Path) {
    checkout(root);
    write_input(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write_input(
        root,
        "crates/example/Cargo.toml",
        "[package]\nname = \"example\"\n",
    );
}

/// The names in `directory`, sorted.
fn names(directory: &Path) -> Vec<String> {
    let mut names = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// The seed keeps what registry crates built and drops member units, outputs and incremental
/// state, so a cold build compiles only the workspace's own crates.
#[test]
fn workspace_sources_seed_keeps_only_registry_units() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    workspace(root);
    let config = printed(
        &setup(root, Failure::None)
            .arg("workspace")
            .output()
            .unwrap(),
    );
    let seed = config.with_file_name("seed");

    assert_eq!(
        names(&seed.join("debug")),
        [".fingerprint", "build", "deps"]
    );
    let registry = ["example-extras-0123456789abcdef", "serde-0123456789abcdef"];
    assert_eq!(names(&seed.join("debug/.fingerprint")), registry);
    assert_eq!(names(&seed.join("debug/build")), registry);
    assert_eq!(
        names(&seed.join("debug/deps")),
        [
            "libexample_extras-0123456789abcdef.rlib",
            "libserde-0123456789abcdef.rlib"
        ]
    );
    assert!(!config.with_file_name("seed-target").exists());
    let builds = fs::read_to_string(root.join("builds")).unwrap();
    let builds = builds.lines().collect::<Vec<_>>();
    assert_eq!(builds.len(), 2);
    assert!(
        builds[0].starts_with("build --locked -p maestro-theme --example browser_author_inputs")
    );
    assert!(builds[1].starts_with("build --locked -p maestro-tooling --bins"));
    assert!(
        builds
            .iter()
            .all(|build| build.ends_with(&format!("--config {}", config.display())))
    );
}

/// A prepared seed is reused, and a failed build leaves none: the next run builds it again
/// without vendoring.
#[test]
fn workspace_sources_seed_is_built_once_and_retried_after_a_failure() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    workspace(root);
    let failed = setup(root, Failure::Build)
        .arg("workspace")
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("build exploded"));
    let sources = fs::read_dir(root.join("target/workspace-sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(!sources.join("seed").exists());

    let config = printed(
        &setup(root, Failure::Vendor)
            .arg("workspace")
            .output()
            .unwrap(),
    );
    assert_eq!(config.parent().unwrap(), sources);
    assert!(sources.join("seed/debug/deps").is_dir());
    let count = |file: &str| fs::read_to_string(root.join(file)).unwrap().lines().count();
    assert_eq!((count("calls"), count("builds")), (1, 3));

    printed(
        &setup(root, Failure::Vendor)
            .arg("workspace")
            .output()
            .unwrap(),
    );
    assert_eq!((count("calls"), count("builds")), (1, 3));
}
