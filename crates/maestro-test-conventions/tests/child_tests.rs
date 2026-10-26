//! Exact child-selection ownership through the public workspace check.
#![cfg(test)]

use crate::support::{self, Workspace};
use maestro_test_conventions::check_workspace;

/// Source loading must enforce checked selection before unrelated source checks.
#[test]
fn exact_child_selection_reaches_workspace_check() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    let path = workspace
        .root
        .path()
        .join(support::native_path("crates/models/src/lib.rs"));
    std::fs::write(&path, "//! Model.\nconst FLAG: &str = r##\"--exact\"##;\n").unwrap();
    assert_eq!(
        check_workspace(workspace.root.path()).unwrap_err(),
        format!(
            "{}:2: exact test selection belongs to maestro-test-process",
            path.display()
        )
    );
    std::fs::write(&path, "//! Model.\n").unwrap();
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
