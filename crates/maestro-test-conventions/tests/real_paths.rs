//! Real-path resolution stays with its owner, through the public workspace check.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// Write one member's source file and compare the complete located diagnostics.
fn check(member: &str, file: &str, contents: &str, expected: &[usize]) {
    let workspace = Workspace::new();
    workspace.foundation(&[member]);
    let path = source(&workspace, member, file, contents);
    let result = if expected.is_empty() {
        Ok(())
    } else {
        Err(expected
            .iter()
            .map(|line| {
                format!(
                    "{}:{line}: production code resolves a real path outside maestro_path::fs; call maestro_path::fs::canonicalize",
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
        "src/lib.rs",
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
    let _ = maestro_path::canonicalize(path);
    let _ = fs::canonicalize(path);
    let _ = maestro_path::other::canonicalize(path);
    let _ = format!("{:?}", fs::canonicalize(path));
}
"#,
        &[2, 6, 7, 8, 9, 10, 12, 13, 14, 15],
    );
}

#[test]
fn the_owner_crate_resolves_real_paths_only_in_its_file_system_module() {
    const RESOLVE: &str = "fn resolve() {\n    let _ = std::fs::canonicalize(\".\");\n}\n";
    check(
        "maestro-path",
        "src/lib.rs",
        &format!("//! Paths.\n\n{RESOLVE}"),
        &[4],
    );
    check(
        "maestro-path",
        "src/fs.rs",
        &format!("//! File-system paths.\n\n{RESOLVE}"),
        &[],
    );
    check(
        "maestro-tools",
        "src/fs.rs",
        &format!("//! Tool files.\n\n{RESOLVE}"),
        &[4],
    );
}

#[test]
fn the_owner_call_and_test_code_are_accepted() {
    check(
        "maestro-tools",
        "src/lib.rs",
        r#"//! Tools.
use std::path::Path;

fn owner(path: &Path) {
    let _ = maestro_path::fs::canonicalize(path);
    let _ = format!("{:?}", maestro_path::fs::canonicalize(path));
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
}
