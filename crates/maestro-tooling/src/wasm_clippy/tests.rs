//! Unit coverage for the browser-target lint: its package selection and the recipe that runs it.

use std::ffi::OsString;

use super::{linted, selection};

/// The recipe step that lints the declared browser crates.
const STEP: &str =
    "@cargo run --quiet --locked -p maestro-tooling --bin development -- wasm-clippy";

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

/// The recipe's two selections read as the whole workspace and as named packages.
#[test]
fn the_recipe_selections_name_browser_crates() {
    assert_eq!(selection(&[OsString::from("--workspace")]).unwrap(), None);
    assert_eq!(
        selection(&[OsString::from("-p"), OsString::from("maestro-lock")]).unwrap(),
        Some(vec!["maestro-lock".to_owned()])
    );
}

/// A selection this lint cannot narrow fails instead of linting another set of crates.
#[test]
fn an_unknown_selection_fails() {
    assert!(selection(&[OsString::from("--exclude"), OsString::from("maestro-app")]).is_err());
}

/// The check recipe lints the declared browser crates with the selection it was given.
#[test]
fn the_check_recipe_lints_the_declared_browser_crates() {
    let justfile = include_str!("../../../../justfile");
    let recipe = justfile
        .split("\n\n")
        .find(|recipe| recipe.lines().any(|line| line.starts_with("_check ")))
        .expect("the check recipe exists");
    assert!(
        recipe
            .lines()
            .any(|line| line.trim() == format!("{STEP} {{{{quote(selection)}}}}")),
        "{STEP} runs in the check recipe:\n{recipe}"
    );
}
