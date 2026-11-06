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
    check(
        "maestro-cli",
        "tests/package_command_checks.rs",
        SEEDED,
        &[],
    );
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
fn builder_declaration_name_is_accepted() {
    check(
        "maestro-app",
        "tests/support/harness.rs",
        "//! Session harness support.\n\nstruct Child;\n\nimpl Child {\n    /// Point the child at one directory.\n    pub fn temp_dir(self, path: &std::path::Path) -> Self {\n        self\n    }\n}\n",
        &[],
    );
}

#[test]
fn declared_item_fn_name_is_accepted() {
    check(
        "maestro-app",
        "tests/support/harness.rs",
        "//! Session harness support.\n\nfn temp_dir() -> std::path::PathBuf {\n    maestro_test_directory::temporary_root()\n}\n",
        &[],
    );
}

#[test]
fn declared_trait_fn_name_is_accepted() {
    check(
        "maestro-app",
        "tests/support/harness.rs",
        "//! Session harness support.\n\ntrait Landable {\n    fn temp_dir(&self) -> std::path::PathBuf;\n}\n",
        &[],
    );
}

#[test]
fn declared_foreign_fn_name_is_accepted() {
    check(
        "maestro-app",
        "tests/support/harness.rs",
        "//! Session harness support.\n\nextern \"C\" {\n    fn temp_dir() -> std::path::PathBuf;\n}\n",
        &[],
    );
}

#[test]
fn declaration_name_does_not_exempt_a_same_line_call() {
    check(
        "maestro-app",
        "tests/support/harness.rs",
        "//! Session harness support.\n\nfn temp_dir() -> std::path::PathBuf { std::env::temp_dir().join(\"maestro-harness\") }\n",
        &[3],
    );
}

#[test]
fn platform_directory_passed_to_a_method_call_is_rejected() {
    check(
        "maestro-app",
        "tests/support/child.rs",
        "//! Child processes.\n\n#[test]\nfn launches_the_selected_child_in_its_own_temporary_directory() {\n    let mut child = maestro_test_process::TestProcess::current(\"name\").unwrap();\n    child.current_dir(std::env::temp_dir());\n}\n",
        &[6],
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

/// Platform directory spellings in the crates added to the migrated scope.
const ADDED: &[(&str, &str, &str, &[usize])] = &[
    (
        "maestro-test-process",
        "tests/environment_groups.rs",
        "//! Group fixtures.\n\nfn marker(test: &str) -> std::path::PathBuf {\n    let path = std::env::temp_dir().join(format!(\"maestro-groups-{test}\"));\n    std::fs::write(&path, \"\").unwrap();\n    path\n}\n",
        &[4],
    ),
    (
        "maestro-catalog",
        "tests/support/common.rs",
        "//! Catalog fixtures.\n\n/// Disposable native file, created exclusively.\npub struct File(pub std::path::PathBuf);\nimpl File {\n    /// Create one file.\n    pub fn new(bytes: &[u8]) -> Self {\n        let path = std::env::temp_dir().join(\"maestro-catalog.json\");\n        std::fs::write(&path, bytes).unwrap();\n        Self(path)\n    }\n}\n",
        &[8],
    ),
    (
        "maestro-process",
        "src/shell/tests/context.rs",
        "//! Shell context fixtures.\n\n/// Reuses the same executable fixtures in a checked child.\nfn in_directory_context(name: &str) {\n    let root = std::env::temp_dir().join(format!(\"maestro-launch-context-{name}\"));\n    std::fs::create_dir_all(&root).unwrap();\n}\n",
        &[5],
    ),
    (
        "maestro-tooling",
        "tests/tooling_commands.rs",
        "//! Tooling commands.\n\n#[test]\nfn compiler_version_runs_outside_the_checkout() {\n    let mut compiler = std::process::Command::new(\"rustc\");\n    compiler.current_dir(std::env::temp_dir()).arg(\"--version\");\n    let _ = compiler;\n}\n",
        &[6],
    ),
    (
        "maestro-tui-crossterm",
        "tests/lifecycle_support/mod.rs",
        "//! Terminal input fixtures.\n\n/// Installs a regular file holding `content` as standard input.\npub fn file_input(content: &[u8]) {\n    let path = std::env::temp_dir().join(\"maestro-terminal-input\");\n    std::fs::write(&path, content).unwrap();\n}\n",
        &[5],
    ),
    (
        "maestro-ipc",
        "tests/support/mod.rs",
        "//! Ipc fixtures.\n\n#[test]\nfn a_request_names_its_own_directory() {\n    let root = std::env::temp_dir();\n    assert!(root.is_absolute());\n}\n",
        &[5],
    ),
    (
        "maestro-lock",
        "tests/replacement.rs",
        "//! Replacement fixtures.\n\n#[test]\nfn a_replacement_replaces_the_previous_file() {\n    let path: std::path::PathBuf = std::env::temp_dir().join(\"maestro-replacement\");\n    std::fs::write(&path, b\"x\").unwrap();\n}\n",
        &[5],
    ),
    (
        "maestro-listing",
        "tests/entry_order.rs",
        "//! Listing order.\n\n#[test]\nfn entries_come_sorted_by_name() {\n    std::fs::create_dir_all(std::env::temp_dir().join(\"maestro-entries\")).unwrap();\n}\n",
        &[5],
    ),
    (
        "maestro-tui",
        "tests/fixtures/completion_native.rs",
        "//! Native completion fixtures.\n\n#[test]\nfn completion_reads_the_authored_root() {\n    let path = std::env::temp_dir().join(\"maestro-completion.json\");\n    std::fs::write(&path, b\"{}\").unwrap();\n}\n",
        &[5],
    ),
    (
        "maestro-workspace",
        "tests/support/snapshot.rs",
        "//! Snapshot fixtures.\n\n#[test]\nfn a_snapshot_roundtrips_through_the_platform_directory() {\n    let root = std::env::temp_dir().join(\"maestro-snapshot\");\n    std::fs::create_dir_all(&root).unwrap();\n}\n",
        &[5],
    ),
    (
        "maestro-credentials",
        "tests/config_values.rs",
        "//! Credential configs.\n\n#[test]\nfn values_read_the_platform_directory() {\n    let value = std::env::temp_dir().to_string_lossy().into_owned();\n    assert!(!value.is_empty());\n}\n",
        &[5],
    ),
    (
        "maestro-tools",
        "tests/streamed_output.rs",
        "//! Streamed output.\n\n#[test]\nfn spilled_output_lands_in_the_platform_directory() {\n    let root = std::env::temp_dir();\n    assert!(root.is_absolute());\n}\n",
        &[5],
    ),
    (
        "maestro-resources",
        "tests/skill_discovery.rs",
        "//! Skill discovery.\n\n#[test]\nfn discovery_runs_from_the_platform_directory() {\n    let cwd = std::env::temp_dir();\n    assert!(cwd.is_absolute());\n}\n",
        &[5],
    ),
    (
        "maestro-chat",
        "tests/extension_editor.rs",
        "//! Extension editor fixtures.\n\n#[test]\nfn the_editor_reads_its_handoff_file() {\n    let path = std::env::temp_dir().join(\"maestro-extension-editor.md\");\n    std::fs::write(&path, b\"draft\").unwrap();\n}\n",
        &[5],
    ),
];

#[test]
fn added_crates_report_their_platform_spelling() {
    for &(member, file, contents, expected) in ADDED {
        check(member, file, contents, expected);
    }
}
