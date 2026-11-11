//! Unit coverage for the doctest phase: package selection and the library check.

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

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
        fs::create_dir_all(root.join(member).join("src")).unwrap();
        fs::write(
            root.join(member).join("Cargo.toml"),
            format!("[package]\nname = '{member}'\nversion = '0.0.0'\nedition = '2024'\n"),
        )
        .unwrap();
        for file in files {
            fs::write(root.join(member).join(file), "fn main() {}\n").unwrap();
        }
    }
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = ['library', 'binary', 'mixed']\nresolver = '3'\n",
    )
    .unwrap();
    MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .unwrap()
}

/// Turn argument text into the OS strings the recipe passes.
fn args(arguments: &[&str]) -> Vec<OsString> {
    arguments.iter().map(OsString::from).collect()
}

/// A package without a library has no doctest phase, so a selection of such packages skips it.
#[test]
fn a_selection_of_packages_without_a_library_runs_no_doctests() {
    let scratch = Scratch::new().unwrap();
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
    let scratch = Scratch::new().unwrap();
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
    let scratch = Scratch::new().unwrap();
    fixture(scratch.path());
    let code = run(
        &args(&["-p", "binary", "--locked"]),
        Path::new(env!("CARGO")),
        scratch.path(),
    )
    .unwrap();
    assert_eq!(code, ExitCode::SUCCESS);
}
