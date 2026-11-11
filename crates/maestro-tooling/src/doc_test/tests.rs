//! Unit coverage for the doctest phase: package selection and the library check.

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use cargo_metadata::{Metadata, MetadataCommand};
use maestro_test_directory::Scratch;

use super::{package_names, run, runs_doctests};

/// Read the metadata of a workspace with a library, a binary and a mixed member.
fn fixture(root: &Path) -> Metadata {
    for (member, files) in [
        ("library", &["src/lib.rs"][..]),
        ("binary", &["src/main.rs"][..]),
        ("mixed", &["src/lib.rs", "src/main.rs"][..]),
    ] {
        fs::create_dir_all(root.join(member).join("src"))
            .expect("the member source directory must be created");
        fs::write(
            root.join(member).join("Cargo.toml"),
            format!("[package]\nname = '{member}'\nversion = '0.0.0'\nedition = '2024'\n"),
        )
        .expect("the member manifest must be written");
        for file in files {
            fs::write(root.join(member).join(file), "fn main() {}\n")
                .expect("the member source file must be written");
        }
    }
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = ['library', 'binary', 'mixed']\nresolver = '3'\n",
    )
    .expect("the workspace manifest must be written");
    MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .expect("cargo metadata must read the fixture")
}

/// Turn argument text into the OS strings the recipe passes.
fn args(arguments: &[&str]) -> Vec<OsString> {
    arguments.iter().map(OsString::from).collect()
}

/// A package without a library has no doctest phase, so a selection of such packages skips it.
#[test]
fn a_selection_of_packages_without_a_library_runs_no_doctests() {
    let scratch = Scratch::new().expect("the scratch directory must be created");
    let metadata = fixture(scratch.path());
    assert!(!runs_doctests(&metadata, &["binary".to_owned()]));
    assert!(runs_doctests(&metadata, &["library".to_owned()]));
    assert!(runs_doctests(&metadata, &["mixed".to_owned()]));
    assert!(runs_doctests(
        &metadata,
        &["binary".to_owned(), "library".to_owned()]
    ));
}

/// A name that is no member goes to Cargo, which reports it.
#[test]
fn an_unknown_package_name_is_left_to_cargo() {
    let scratch = Scratch::new().expect("the scratch directory must be created");
    let metadata = fixture(scratch.path());
    assert!(runs_doctests(&metadata, &["missing".to_owned()]));
    assert!(runs_doctests(&metadata, &["bin*".to_owned()]));
}

/// Every Cargo package spelling names its package; filters and the arguments after `--` do not.
#[test]
fn every_package_spelling_names_its_package() {
    assert_eq!(
        package_names(&args(&[
            "-p",
            "one",
            "--package",
            "two",
            "--package=three",
            "-pfour",
            "filter",
            "--",
            "-p",
            "after",
        ])),
        ["one", "two", "three", "four"]
    );
    assert!(package_names(&args(&["filter", "--", "--ignored", "-p", "name"])).is_empty());
}

/// Naming a binary-only package passes without running Cargo's doctests, which refuse it.
#[test]
fn naming_a_binary_only_package_passes_the_doctest_phase() {
    let scratch = Scratch::new().expect("the scratch directory must be created");
    fixture(scratch.path());
    // `run` passes `--locked` to Cargo, so the workspace needs its lock file.
    let lock = Command::new(env!("CARGO"))
        .arg("generate-lockfile")
        .current_dir(scratch.path())
        .status()
        .expect("cargo generate-lockfile must start");
    assert!(lock.success());
    let code = run(
        &args(&["-p", "binary"]),
        Path::new(env!("CARGO")),
        scratch.path(),
    )
    .expect("the doctest phase must run");
    assert_eq!(code, ExitCode::SUCCESS);
}
