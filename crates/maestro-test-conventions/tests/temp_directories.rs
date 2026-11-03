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
fn check(member: &str, file: &str, contents: &str, expected: &[usize]) {
    let workspace = Workspace::new();
    workspace.foundation(&[member]);
    let path = source(&workspace, member, file, contents);
    if file.starts_with("tests/") {
        workspace.integration(member);
    }
    let result = if expected.is_empty() {
        Ok(())
    } else {
        Err(expected
            .iter()
            .map(|line| {
                format!(
                    "{}:{line}: test-owned code names the platform temporary directory; use maestro_test_directory::Scratch or maestro_test_directory::temporary_root, and add a new crate to TEST_DIRECTORY_CALLERS",
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
    check("maestro-daemon", "src/logging/tests.rs", SEEDED, &[4]);
}

#[test]
fn inline_test_module_in_production_file_is_rejected() {
    check(
        "maestro-daemon",
        "src/lib.rs",
        "//! Daemon.\n\nfn production() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn writes_a_directory() {\n        let path = || std::env::temp_dir().join(\"maestro-inline\");\n        std::fs::create_dir_all(path()).unwrap();\n    }\n}\n",
        &[9],
    );
}

#[test]
fn test_gated_impl_helper_is_rejected() {
    check(
        "maestro-session",
        "src/session_manager/mod.rs",
        "//! Session manager.\n\nstruct Root;\n\n#[cfg(test)]\nimpl Root {\n    fn path(&self) -> std::path::PathBuf {\n        std::env::temp_dir().join(\"maestro-impl\")\n    }\n}\n",
        &[8],
    );
}

#[test]
fn typed_binding_in_test_code_is_rejected() {
    check(
        "maestro-watch",
        "tests/native_notifications.rs",
        "//! Watch.\n\n#[test]\nfn watches_a_subdirectory() {\n    let base: std::path::PathBuf = std::env::temp_dir().join(\"maestro-watch\");\n    let path: std::path::PathBuf = base.join(\"watched\");\n    std::fs::create_dir_all(&path).unwrap();\n}\n",
        &[5],
    );
}

#[test]
fn renamed_import_is_rejected() {
    check(
        "maestro-session",
        "tests/session_files.rs",
        "//! Session files.\n\nuse std::env::temp_dir as platform;\n\n#[test]\nfn publishes_a_file() {\n    assert!(platform().is_absolute());\n}\n",
        &[3],
    );
}

#[test]
fn import_in_production_scope_feeding_test_code_is_rejected() {
    check(
        "maestro-daemon",
        "src/pane/mod.rs",
        "//! Pane.\n\nuse std::env::temp_dir as platform;\n\n#[cfg(test)]\nfn socket_path() -> std::path::PathBuf {\n    platform().join(\"maestro-test.sock\")\n}\n",
        &[3],
    );
}

#[test]
fn macro_argument_is_rejected() {
    check(
        "maestro-daemon",
        "src/pane/runtime/tests.rs",
        "//! Runtime.\n\n#[test]\nfn reports_the_platform_directory() {\n    assert!(std::env::temp_dir().is_absolute());\n}\n",
        &[5],
    );
}

#[test]
fn discarded_owner_call_does_not_hide_a_named_directory() {
    check(
        "maestro-daemon",
        "src/persist/tests.rs",
        "//! Persist.\n\n#[test]\nfn saves_a_snapshot() {\n    let _ = maestro_test_directory::Scratch::new().unwrap();\n    std::fs::create_dir_all(std::env::temp_dir().join(\"maestro-snapshot\")).unwrap();\n}\n",
        &[6],
    );
}

#[test]
fn owner_parent_from_the_owner_root_is_accepted() {
    check(
        "maestro-daemon",
        "src/persist/tests.rs",
        "//! Persist.\n\n#[test]\nfn saves_a_snapshot() {\n    let scratch = maestro_test_directory::Scratch::new_in(&maestro_test_directory::temporary_root()).unwrap();\n    std::fs::create_dir(scratch.path().join(\"work\")).unwrap();\n}\n",
        &[],
    );
}

#[test]
fn platform_directory_observation_is_accepted() {
    check(
        "maestro-daemon",
        "src/pane/runtime/tests.rs",
        "//! Runtime.\n\n#[test]\nfn reports_the_platform_directory() {\n    let cwd = maestro_test_directory::temporary_root();\n    assert!(cwd.is_absolute());\n}\n",
        &[],
    );
}

#[test]
fn uncreated_socket_path_is_accepted() {
    check(
        "maestro-daemon",
        "src/pane/runtime/tests.rs",
        "//! Runtime.\n\nfn paths() -> std::path::PathBuf {\n    maestro_test_directory::temporary_root().join(\"maestro-test.sock\")\n}\n\n#[test]\nfn reports_the_socket_path() {\n    assert!(paths().is_absolute());\n}\n",
        &[],
    );
}

#[test]
fn single_temporary_file_is_accepted() {
    check(
        "maestro-session",
        "tests/session_files.rs",
        "//! Session files.\n\n#[test]\nfn publishes_a_file() {\n    let path = maestro_test_directory::temporary_root().join(\"maestro-published.jsonl\");\n    std::fs::write(&path, b\"line\").unwrap();\n}\n",
        &[],
    );
}

#[test]
fn owner_path_shadowing_its_platform_parent_is_accepted() {
    check(
        "maestro-daemon",
        "src/persist/tests.rs",
        "//! Persist.\n\n#[test]\nfn saves_a_snapshot() {\n    let root = maestro_test_directory::temporary_root();\n    let scratch = maestro_test_directory::Scratch::new_in(&root).unwrap();\n    let root = scratch.path();\n    std::fs::create_dir(root.join(\"work\")).unwrap();\n}\n",
        &[],
    );
}

#[test]
fn production_temporary_directory_is_accepted() {
    check(
        "maestro-daemon",
        "src/pane/mod.rs",
        "//! Pane.\n\nfn socket_path() -> std::path::PathBuf {\n    std::env::temp_dir().join(\"maestro-test.sock\")\n}\n",
        &[],
    );
}

#[test]
fn unmigrated_crate_keeps_its_own_directories_until_its_batch() {
    check("maestro-tools", "tests/tooling.rs", SEEDED, &[]);
}

/// The detection helper that returned a platform directory it never created.
const DETECTION: &str = "//! Agent process trials.\n\n#[test]\nfn wraps_wrapped_agents_without_a_job_copy() {\n    let unique = format!(\"maestro-detection-{}\", std::time::SystemTime::now());\n    let path: std::path::PathBuf = std::env::temp_dir().join(unique);\n    assert!(path.to_str().unwrap().starts_with(\"maestro\"));\n}\n";

#[test]
fn hand_named_detection_directory_is_rejected() {
    check(
        "maestro-detection",
        "src/agents/process/tests.rs",
        DETECTION,
        &[6],
    );
}

#[test]
fn extensions_wasmtime_names_an_uncreated_trace() {
    check(
        "maestro-extensions-wasmtime",
        "tests/canonical_commands.rs",
        "//! Canonical fixtures.\n\n#[test]\nfn binds_before_the_jsonl_trace_exists() {\n    let file = std::env::temp_dir().join(\"maestro-canonical.jsonl\");\n    assert!(!file.exists());\n}\n",
        &[5],
    );
}

#[test]
fn terminal_upload_writes_the_platform_directory() {
    check(
        "maestro-terminal",
        "tests/graphics_placements.rs",
        "//! Graphics placements.\n\n#[test]\nfn uploads_a_raw_pixel_through_the_apc_route() {\n    let path = std::env::temp_dir().join(\"maestro-graphics-placement.rgba\");\n    std::fs::write(&path, [255, 0, 0, 255]).unwrap();\n}\n",
        &[5],
    );
}

#[test]
fn models_write_authored_scripts_into_the_platform_directory() {
    check(
        "maestro-models",
        "src/providers/http/client/tests/pac.rs",
        "//! Provider client fixtures.\n\n#[test]\nfn pac_sources_read_authored_scripts() {\n    let path = std::env::temp_dir().join(\"maestro-pac-retain\");\n    std::fs::write(&path, b\"script\").unwrap();\n}\n",
        &[5],
    );
}

#[test]
fn app_canonicalizes_the_platform_directory() {
    check(
        "maestro-app",
        "src/configuration/migrations/tests.rs",
        "//! Settlement tests.\n\n/// Create an isolated directory for one filesystem witness.\nfn root() -> std::path::PathBuf {\n    std::fs::canonicalize(std::env::temp_dir()).unwrap()\n}\n",
        &[5],
    );
}

#[test]
fn test_process_builder_method_call_is_accepted() {
    check(
        "maestro-app",
        "tests/support/session_harness.rs",
        "//! Session harness support.\n\n#[test]\nfn launches_the_selected_child_in_its_own_temporary_directory() {\n    let root = maestro_test_directory::Scratch::new().unwrap();\n    let child = maestro_test_process::TestProcess::current(\"name\")\n        .unwrap()\n        .nocapture()\n        .temp_dir(root.path());\n    let _ = child;\n}\n",
        &[],
    );
}

#[test]
fn packages_reads_the_platform_directory_into_a_setting() {
    check(
        "maestro-packages",
        "src/package_manager/tests.rs",
        "//! Package manager fixtures.\n\n#[test]\nfn manager_cwd_stays_the_observed_platform_directory() {\n    let cwd: String = std::env::temp_dir().to_string_lossy().into_owned();\n    assert!(!cwd.is_empty());\n}\n",
        &[5],
    );
}
