//! Modules are named for what they hold, through the public workspace check.
#![cfg(test)]

use crate::support::{Workspace, source};
use maestro_test_conventions::check_workspace;

/// The note every diagnostic ends with.
const ADVICE: &str = "of maestro-text is named for where code was put; name it for what it holds (modules that keep such a name: EXEMPT in bucket_modules.rs)";

#[test]
fn a_bucket_named_module_is_rejected_with_its_file_and_name() {
    let workspace = Workspace::with_text_member();
    let root = source(
        &workspace,
        "text",
        "src/lib.rs",
        "//! Text.\nmod utils;\nmod width {\n    mod helpers {}\n}\n",
    );
    source(&workspace, "text", "src/utils.rs", "//! Utilities.\n");
    let cells = source(
        &workspace,
        "text",
        "src/width/shared/cells.rs",
        "//! Cells.\n",
    );
    let (root, shared) = (root.display(), cells.parent().unwrap().display());

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([
            format!("{root}:2: module `utils` {ADVICE}"),
            format!("{root}:4: module `width::helpers` {ADVICE}"),
            format!("{shared}: module `width::shared` {ADVICE}"),
        ]
        .join("\n"))
    );
}

#[test]
fn the_same_tree_passes_once_the_modules_say_what_they_hold() {
    let workspace = Workspace::with_text_member();
    source(
        &workspace,
        "text",
        "src/lib.rs",
        "//! Text.\nmod parsing;\nmod width {\n    mod columns {}\n}\n",
    );
    source(&workspace, "text", "src/parsing.rs", "//! Parsing.\n");
    source(
        &workspace,
        "text",
        "src/width/cells/columns.rs",
        "//! Columns.\n",
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn a_module_in_the_exemption_table_keeps_its_name() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-interchange"]);
    source(
        &workspace,
        "maestro-interchange",
        "src/lib.rs",
        "//! Shared records.\npub mod types;\n",
    );
    source(
        &workspace,
        "maestro-interchange",
        "src/types.rs",
        "//! Records.\n",
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
