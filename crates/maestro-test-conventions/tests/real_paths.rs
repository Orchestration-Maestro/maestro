//! Real-path resolution stays with its owner, through the public workspace check.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// Write one member's source and compare the complete located diagnostics.
fn check(member: &str, contents: &str, expected: &[usize]) {
    let workspace = Workspace::new();
    workspace.foundation(&[member]);
    let path = source(&workspace, member, "src/lib.rs", contents);
    let result = if expected.is_empty() {
        Ok(())
    } else {
        Err(expected
            .iter()
            .map(|line| {
                format!(
                    "{}:{line}: production code resolves a real path outside maestro-realpath; call maestro_realpath::canonicalize",
                    path.display()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    };
    assert_eq!(check_workspace(workspace.root.path()), result);
}

#[test]
fn production_real_path_resolution_outside_the_owner_is_rejected() {
    check(
        "maestro-tools",
        r#"//! Tools.
use std::fs::canonicalize as real;
use std::path::Path;

fn spellings(path: &Path) {
    let _ = std::fs::canonicalize(path);
    let _ = path.canonicalize();
    let _ = tokio::fs::canonicalize(path);
    let _ = Some(path).map(Path::canonicalize);
    let _ = format!("{:?}", std::fs::canonicalize(path));
    let _ = real(path);
}
"#,
        &[2, 6, 7, 8, 9, 10],
    );
}

#[test]
fn the_owner_call_and_test_code_are_accepted() {
    check(
        "maestro-tools",
        r#"//! Tools.
use std::path::Path;

fn owner(path: &Path) {
    let _ = maestro_realpath::canonicalize(path);
    let _ = format!("{:?}", maestro_realpath::canonicalize(path));
}

#[cfg(test)]
mod tests {
    #[test]
    fn expectation() {
        let _ = std::fs::canonicalize(".");
    }
}
"#,
        &[],
    );
    check(
        "maestro-realpath",
        "//! Real paths.\n\nfn resolve() {\n    let _ = std::fs::canonicalize(\".\");\n}\n",
        &[],
    );
}
