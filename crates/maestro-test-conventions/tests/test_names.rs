//! Behaviour names checked through the workspace boundary.
#![cfg(test)]

use crate::support::{Workspace, source};
use maestro_test_conventions::check_workspace;
use std::path::Path;

/// Renders one located diagnostic for a rejected test name.
fn diagnostic(path: &Path, line: usize, name: &str) -> String {
    format!(
        "{}:{line}: test `{name}` must describe behaviour without a maestro_, test_ or numbered issue_ label",
        path.display()
    )
}

#[test]
fn behaviour_names_without_numbered_labels_are_accepted() {
    let workspace = Workspace::with_text_member();
    source(
        &workspace,
        "text",
        "src/lib.rs",
        "//! Text.\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn parses_x() {}\n    #[runner::test]\n    fn keeps_maestro_text_and_test_words() {}\n    #[test]\n    fn reads_issue_text() {}\n    #[test]\n    fn reads_issue_() {}\n}\n",
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn unattributed_helpers_keep_their_names() {
    let workspace = Workspace::with_text_member();
    source(
        &workspace,
        "text",
        "src/lib.rs",
        "//! Text.\n#[cfg(test)]\nmod tests {\n    fn maestro_fixture() {}\n    fn test_fixture() {}\n    fn fixture_issue_123() {}\n}\n",
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn prefixed_test_names_are_rejected_with_their_locations() {
    for (contents, lines) in [
        (
            "//! Text.\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn maestro_parses_x() {}\n    #[tokio::test]\n    async fn test_parses_y() {}\n    #[runner::test]\n    fn maestro_parses_z() {}\n}\n",
            [5, 7, 9],
        ),
        (
            "//! Text.\n#[test]\nfn maestro_parses_x() {}\n#[tokio::test]\nasync fn test_parses_y() {}\n#[runner::test]\nfn maestro_parses_z() {}\n",
            [3, 5, 7],
        ),
    ] {
        let workspace = Workspace::with_text_member();
        let path = source(&workspace, "text", "src/lib.rs", contents);
        let expected = lines
            .into_iter()
            .zip(["maestro_parses_x", "test_parses_y", "maestro_parses_z"])
            .map(|(line, name)| diagnostic(&path, line, name))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(check_workspace(workspace.root.path()), Err(expected));
    }
}

#[test]
fn raw_identifiers_cannot_hide_redundant_prefixes() {
    let workspace = Workspace::with_text_member();
    let path = source(
        &workspace,
        "text",
        "src/tests.rs",
        "//! Text.\n#![cfg(test)]\n#[test]\nfn r#maestro_parses_raw() {}\n#[r#test]\nfn r#test_parses_raw() {}\n",
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([(4, "maestro_parses_raw"), (6, "test_parses_raw")]
            .map(|(line, name)| diagnostic(&path, line, name))
            .join("\n"))
    );
}

#[test]
fn numbered_labels_are_rejected_in_every_test_layout() {
    for relative in [
        "src/tests.rs",
        "src/nested/tests/cases.rs",
        "tests/cases.rs",
    ] {
        let workspace = Workspace::with_text_member();
        let path = source(
            &workspace,
            "text",
            relative,
            "//! Cases.\n#[test]\nfn reads_issue_42_data() {}\n#[runner::test]\nfn reads_issue_9x() {}\n#[test]\nfn issue_0() {}\n",
        );
        if relative == "tests/cases.rs" {
            workspace.integration("text");
        }
        let expected = [
            (3, "reads_issue_42_data"),
            (5, "reads_issue_9x"),
            (7, "issue_0"),
        ]
        .map(|(line, name)| diagnostic(&path, line, name))
        .join("\n");

        assert_eq!(check_workspace(workspace.root.path()), Err(expected));
    }
}
