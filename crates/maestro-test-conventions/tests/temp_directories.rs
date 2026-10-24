//! Test-owned temporary directory ownership through the public workspace check.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// The removed daemon logging helper and its two ownership calls.
const SEEDED: &str = r#"use std::path::PathBuf;
fn temp_log_path(name: &str) -> PathBuf {
    let unique = format!("maestro-logging-tests-{name}");
    std::env::temp_dir().join(unique).join("maestro.log")
}

#[test]
fn log_rotation_shifts_generations_in_order() {
    let path = temp_log_path("rotate");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
"#;

/// Write one member's source and compare the complete located diagnostics.
fn check(member: &str, file: &str, contents: &str, expected: &[(usize, &str)]) {
    let workspace = Workspace::new();
    workspace.foundation(&[member]);
    let path = source(&workspace, member, file, contents);
    let result = if expected.is_empty() {
        Ok(())
    } else {
        Err(expected
            .iter()
            .map(|(line, call)| {
                format!(
                    "{}:{line}: test-owned temporary directory: {call}; use maestro_test_directory::Scratch instead, and add a new crate to TEST_DIRECTORY_CALLERS",
                    path.display()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    };
    assert_eq!(check_workspace(workspace.root.path()), result);
}

#[test]
fn hand_named_daemon_directory_is_rejected() {
    check(
        "maestro-daemon",
        "src/logging/tests.rs",
        SEEDED,
        &[(10, "create_dir_all"), (11, "remove_dir_all")],
    );
}

#[test]
fn hand_named_session_directory_is_rejected() {
    check(
        "maestro-session",
        "tests/session_files.rs",
        "//! Session files.\n\n#[test]\nfn publishes_a_file() {\n    let path = std::env::temp_dir().join(\"maestro-session\");\n    std::fs::create_dir(&path).unwrap();\n}\n",
        &[(6, "create_dir")],
    );
}

#[test]
fn derived_subdirectory_creation_is_rejected() {
    check(
        "maestro-watch",
        "tests/native_notifications.rs",
        "//! Watch.\n\n#[test]\nfn watches_a_subdirectory() {\n    let base = std::env::temp_dir().join(\"maestro-watch\");\n    let path = base.join(\"watched\");\n    std::fs::create_dir_all(&path).unwrap();\n}\n",
        &[(7, "create_dir_all")],
    );
}

#[test]
fn discarded_owner_call_does_not_hide_an_owned_directory() {
    check(
        "maestro-daemon",
        "src/persist/tests.rs",
        "//! Persist.\n\n#[test]\nfn saves_a_snapshot() {\n    let _ = maestro_test_directory::Scratch::new().unwrap();\n    let root = std::env::temp_dir().join(\"maestro-snapshot\");\n    std::fs::create_dir_all(&root).unwrap();\n}\n",
        &[(7, "create_dir_all")],
    );
}

#[test]
fn owner_parent_from_the_platform_directory_is_accepted() {
    check(
        "maestro-daemon",
        "src/persist/tests.rs",
        "//! Persist.\n\n#[test]\nfn saves_a_snapshot() {\n    let scratch = maestro_test_directory::Scratch::new_in(&std::env::temp_dir()).unwrap();\n    std::fs::create_dir(scratch.path().join(\"work\")).unwrap();\n}\n",
        &[],
    );
}

#[test]
fn platform_directory_observation_is_accepted() {
    check(
        "maestro-daemon",
        "src/pane/runtime/tests.rs",
        "//! Runtime.\n\n#[test]\nfn reports_the_platform_directory() {\n    let cwd = std::env::temp_dir();\n    assert!(cwd.is_absolute());\n}\n",
        &[],
    );
}

#[test]
fn uncreated_socket_path_is_accepted() {
    check(
        "maestro-daemon",
        "src/pane/runtime/tests.rs",
        "//! Runtime.\n\nfn paths() -> std::path::PathBuf {\n    std::env::temp_dir().join(\"maestro-test.sock\")\n}\n\n#[test]\nfn reports_the_socket_path() {\n    assert!(paths().is_absolute());\n}\n",
        &[],
    );
}

#[test]
fn single_temporary_file_is_accepted() {
    check(
        "maestro-session",
        "tests/session_files.rs",
        "//! Session files.\n\n#[test]\nfn publishes_a_file() {\n    let path = std::env::temp_dir().join(\"maestro-published.jsonl\");\n    std::fs::write(&path, b\"line\").unwrap();\n}\n",
        &[],
    );
}

#[test]
fn unmigrated_crate_keeps_its_own_directories_until_its_batch() {
    check("maestro-tools", "tests/tooling.rs", SEEDED, &[]);
}
