//! Unit coverage for the browser-target lint: the command it runs and its package selection.

use std::ffi::OsString;

use super::{arguments, linted, runtime_arguments, selection};

/// A package selection lints only its declared browser crates.
#[test]
fn a_package_selection_lints_only_declared_browser_crates() {
    let declared = vec!["maestro-app".to_owned(), "maestro-lock".to_owned()];
    let names = vec!["maestro-lock".to_owned(), "maestro-daemon".to_owned()];
    assert_eq!(linted(&declared, Some(&names)), ["maestro-lock"]);
}

/// The whole workspace selection lints every declared browser crate in declaration order.
#[test]
fn the_workspace_selection_lints_every_declared_browser_crate() {
    let declared = vec!["maestro-lock".to_owned(), "maestro-app".to_owned()];
    assert_eq!(linted(&declared, None), ["maestro-lock", "maestro-app"]);
}

/// The lint runs shared CI's command, with one package argument per declared crate.
#[test]
fn the_browser_lint_runs_the_shared_ci_command() {
    let command: Vec<String> = arguments(&["maestro-lock", "maestro-app"])
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        command,
        [
            "clippy",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "maestro-lock",
            "-p",
            "maestro-app",
            "--",
            "-D",
            "warnings",
        ]
    );
}

/// Every Cargo package form reads as the packages it names.
#[test]
fn every_cargo_package_form_names_its_packages() {
    let lock = || Some(vec!["maestro-lock".to_owned()]);
    assert_eq!(selection(&[OsString::from("--workspace")]).unwrap(), None);
    assert_eq!(
        selection(&[OsString::from("-p"), OsString::from("maestro-lock")]).unwrap(),
        lock()
    );
    assert_eq!(
        selection(&[OsString::from("-pmaestro-lock")]).unwrap(),
        lock()
    );
    assert_eq!(
        selection(&[OsString::from("--package"), OsString::from("maestro-lock")]).unwrap(),
        lock()
    );
    assert_eq!(
        selection(&[OsString::from("--package=maestro-lock")]).unwrap(),
        lock()
    );
}

/// A selection this lint cannot narrow to a declared crate fails instead of passing.
#[test]
fn an_unknown_selection_fails() {
    assert!(selection(&[OsString::from("--exclude"), OsString::from("maestro-app")]).is_err());
    assert!(selection(&[OsString::from("-pmaestro-*")]).is_err());
    assert!(selection(&[OsString::from("--package=maestro-lock@0.1.0")]).is_err());
    assert!(selection(&[OsString::from("-p")]).is_err());
}

/// The runtime package, outside the workspace, is linted by manifest path for the target it is
/// built for, with warnings denied.
#[test]
fn the_runtime_package_is_linted_by_manifest_path_for_its_target() {
    let command: Vec<String> = runtime_arguments()
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    assert!(
        command
            .windows(2)
            .any(|pair| pair == ["--manifest-path", "crates/maestro-web/runtime/Cargo.toml"])
    );
    assert!(
        command
            .windows(2)
            .any(|pair| pair == ["--target", "wasm32-unknown-emscripten"]),
        "{command:?}"
    );
    assert!(command.ends_with(&["--".to_owned(), "-D".to_owned(), "warnings".to_owned()]));
}
