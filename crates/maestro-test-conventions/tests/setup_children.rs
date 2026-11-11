//! Setup children in tests clear the prepared bundle their run inherits.

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// A workspace whose text crate holds the supplied test sources.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-text"]);
    workspace
}

/// A test that starts the setup tool over `command` and leaves `extra` after it.
fn setup_child(command: &str, extra: &str) -> String {
    format!(
        "//! Setup children.\n/// Runs the setup this test supplies.\n#[test]\nfn starts_setup() {{\n    let mut command = std::process::Command::new(\"development\");\n    {command}\n{extra}}}\n"
    )
}

#[test]
fn setup_child_without_the_helper_is_rejected() {
    let workspace = workspace();
    let path = source(
        &workspace,
        "maestro-text",
        "tests/setup_child.rs",
        &setup_child(
            "command.args([\"nextest-setup\", \"author-component\"]);",
            "",
        ),
    );
    workspace.integration("maestro-text");

    let error = check_workspace(workspace.root.path()).unwrap_err();

    assert!(error.contains("without_prepared_bundles"), "{error}");
    assert!(error.contains(&path.display().to_string()), "{error}");
}

#[test]
fn setup_child_that_clears_its_bundle_passes() {
    let workspace = workspace();
    source(
        &workspace,
        "maestro-text",
        "tests/setup_child.rs",
        &setup_child(
            "command.args([\"nextest-setup\", \"author-component\"]);\n    maestro_test_process::without_prepared_bundles(&mut command);",
            "",
        ),
    );
    workspace.integration("maestro-text");

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn setup_child_cleared_through_a_function_of_the_source_passes() {
    let workspace = workspace();
    source(
        &workspace,
        "maestro-text",
        "tests/setup_child.rs",
        &setup_child(
            "command.args([\"nextest-setup\", \"author-component\"]);\n    prepared(&mut command);",
            "}\n\n/// Runs the prepared command the test supplies.\nfn prepared(command: &mut std::process::Command) {\n    maestro_test_process::without_prepared_bundles(command);\n",
        ),
    );
    workspace.integration("maestro-text");

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn production_code_that_names_the_setup_entry_passes() {
    let workspace = workspace();
    source(
        &workspace,
        "maestro-text",
        "src/setup.rs",
        "//! Setup entries.\n/// The setup entry nextest runs before a test.\npub const ENTRY: &str = \"nextest-setup\";\n",
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
