//! How the ls definition and tool list directories through replaceable operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::ls_support;
use crate::read_support;

#[cfg(unix)]
use ls_support::check_native;
#[cfg(unix)]
use ls_support::{Case, assert_effects, execute};
use ls_support::{Scripted, cases, check_execute, execute_input};
use maestro_models::Cancellation;
use maestro_tools::{
    LsToolDetails, LsToolInput, LsToolOptions, create_ls_tool_definition,
    output::truncate::{TruncationBy, TruncationResult},
};
use read_support::isolated;
#[cfg(unix)]
use read_support::{Fixture, Scratch};
use serde_json::json;
use std::{collections::BTreeSet, sync::Arc};

/// Run every recorded case of `test` over controlled operations.
fn check_all(test: &str) {
    cases(test).iter().for_each(check_execute);
}

#[test]
fn ls_definition_keeps_schema_metadata_and_optional_inputs() {
    let case = &cases("ls_definition_keeps_schema_metadata_and_optional_inputs")[0];
    let tool = create_ls_tool_definition::<()>("/work".into(), LsToolOptions::default());
    let expected = &case.expected;
    assert_eq!(tool.definition.name, expected["name"].as_str().unwrap());
    assert_eq!(tool.label, expected["label"].as_str().unwrap());
    assert_eq!(
        tool.definition.description,
        expected["description"].as_str().unwrap()
    );
    assert_eq!(
        tool.prompt_snippet.as_deref(),
        expected["promptSnippet"].as_str()
    );
    assert_eq!(*tool.definition.parameters, expected["parameters"]);
    assert!(tool.prompt_guidelines.is_none() && tool.prepare_arguments.is_none());
    assert!(tool.execution_mode.is_none() && tool.render_shell.is_none());

    let blank: LsToolInput = serde_json::from_value(json!({})).unwrap();
    assert_eq!((blank.path, blank.limit), (None, None));
    let nulls: LsToolInput = serde_json::from_value(json!({"path": null, "limit": null})).unwrap();
    assert_eq!((nulls.path, nulls.limit), (None, None));
    let huge: LsToolInput = serde_json::from_value(json!({"limit": 1e21})).unwrap();
    assert_eq!(huge.limit, Some(1e21));

    let absent = LsToolDetails {
        truncation: None,
        entry_limit_reached: None,
    };
    assert_eq!(serde_json::to_value(absent).unwrap(), json!({}));
    let reached = LsToolDetails {
        truncation: Some(TruncationResult {
            content: "x".into(),
            truncated: true,
            truncated_by: Some(TruncationBy::Bytes),
            total_lines: 2,
            total_bytes: 3,
            output_lines: 1,
            output_bytes: 1,
            last_line_partial: false,
            first_line_exceeds_limit: false,
            max_lines: 9,
            max_bytes: 4,
        }),
        entry_limit_reached: Some(1.5),
    };
    let value = serde_json::to_value(reached).unwrap();
    assert_eq!(value["entryLimitReached"], json!(1.5));
    assert_eq!(value["truncation"]["truncatedBy"], json!("bytes"));
    assert_eq!(value["truncation"]["firstLineExceedsLimit"], json!(false));
}

#[test]
fn ls_factory_wraps_supplied_operations() {
    check_all("ls_factory_wraps_supplied_operations");
}

#[test]
fn ls_default_and_authored_paths_reach_operations() {
    check_all("ls_default_and_authored_paths_reach_operations");
    let idle = Arc::new(Scripted::new(
        &ls_support::ExecuteInput::default(),
        &Cancellation::new(),
    ));
    let options = LsToolOptions {
        operations: Some(Arc::clone(&idle) as Arc<dyn maestro_tools::LsOperations>),
    };
    let _tool = create_ls_tool_definition::<()>("relative/cwd".into(), options);
    assert!(idle.log().is_empty());
}

#[test]
fn ls_sort_is_stable_case_insensitive_locale_order() {
    let test = "ls_sort_is_stable_case_insensitive_locale_order";
    let all = cases(test);
    let locales: BTreeSet<String> = all.iter().map(|c| execute_input(c).locale()).collect();
    for locale in locales {
        let env = [
            ("LC_ALL", Some(locale.as_str())),
            ("LC_MESSAGES", None),
            ("LANG", None),
        ];
        if isolated(test, &locale, &env) {
            all.iter()
                .filter(|case| execute_input(case).locale() == locale)
                .for_each(check_execute);
        }
    }
}

#[test]
fn ls_limit_counts_only_successfully_statted_entries() {
    check_all("ls_limit_counts_only_successfully_statted_entries");
}

#[test]
fn ls_limits_keep_zero_negative_fractional_boundaries() {
    check_all("ls_limits_keep_zero_negative_fractional_boundaries");
}

#[test]
fn ls_default_limit_and_exact_boundary_notices() {
    check_all("ls_default_limit_and_exact_boundary_notices");
}

#[test]
fn ls_byte_cap_has_no_separate_line_limit() {
    check_all("ls_byte_cap_has_no_separate_line_limit");
}

#[test]
fn ls_byte_and_entry_notices_keep_order_and_details() {
    check_all("ls_byte_and_entry_notices_keep_order_and_details");
}

#[test]
fn ls_empty_and_unstattable_directories_have_no_details() {
    check_all("ls_empty_and_unstattable_directories_have_no_details");
}

#[test]
fn ls_path_and_directory_failures_keep_precedence() {
    check_all("ls_path_and_directory_failures_keep_precedence");
}

#[test]
fn ls_root_operation_errors_keep_catch_boundaries() {
    check_all("ls_root_operation_errors_keep_catch_boundaries");
}

#[test]
fn ls_cancellation_rejects_before_and_during_operations() {
    let mut all = cases("ls_cancellation_rejects_before_and_during_operations");
    all.sort_by_key(|case| !execute_input(case).is_pending());
    assert_eq!(all.len(), 13);
    all.iter().for_each(check_execute);
}

/// A temporary directory with the entries the native listing cases name.
#[cfg(unix)]
fn native_root() -> Scratch {
    let root = Scratch::new().unwrap();
    for dir in [".git", ".hidden-dir"] {
        std::fs::create_dir(root.path().join(dir)).unwrap();
    }
    for file in [
        ".gitignore",
        ".hidden-file",
        "A",
        "a",
        "e\u{301}",
        "\u{e9}",
        "ignored",
    ] {
        root.file(file, b"");
    }
    root.file(".gitignore", b"ignored\n");
    std::os::unix::fs::symlink(".git", root.path().join("linked-dir")).unwrap();
    std::os::unix::fs::symlink("missing-target", root.path().join("dangling")).unwrap();
    root
}

#[cfg(target_os = "linux")]
#[test]
fn ls_lists_hidden_ignored_and_linked_entries() {
    let root = native_root();
    for case in cases("ls_lists_hidden_ignored_and_linked_entries") {
        check_native(&case, &root.text());
    }
}

#[cfg(unix)]
#[test]
fn ls_native_missing_and_file_paths_fail() {
    let root = native_root();
    for case in cases("ls_native_missing_and_file_paths_fail") {
        check_native(&case, &root.text());
    }
}

#[cfg(unix)]
#[test]
fn ls_native_non_utf8_names_follow_stat_filtering() {
    use std::os::unix::ffi::OsStrExt;
    let root = Scratch::new().unwrap();
    let invalid = root.path().join("invalid");
    std::fs::create_dir(&invalid).unwrap();
    std::fs::write(invalid.join("ok"), b"").unwrap();
    let bad = std::ffi::OsStr::from_bytes(b"bad-\xff");
    if !root.keeps_distinct(&[bad]).unwrap() {
        eprintln!(
            "the listing needs a host that stores the name bytes {:?}",
            bad.as_bytes()
        );
        return;
    }
    std::fs::write(invalid.join(bad), b"").unwrap();
    for case in cases("ls_native_non_utf8_names_follow_stat_filtering") {
        check_native(&case, &root.text());
    }
}

#[cfg(unix)]
#[test]
fn ls_resolution_reads_host_cwd_only_when_needed() {
    let test = "ls_resolution_reads_host_cwd_only_when_needed";
    if !isolated(test, "removed", &[]) {
        return;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    scratch.enter_removed("removed").unwrap();
    assert!(std::env::current_dir().is_err());
    for case in cases(test) {
        assert!(execute_input(&case).removes_cwd());
        if case.id == "required-failure" {
            require_failure(&case);
        } else {
            check_execute(&case);
        }
    }
}

/// A relative operand needs the removed directory: the call fails before any operation, with
/// the platform's own message.
#[cfg(unix)]
fn require_failure(case: &Case) {
    let outcome = execute(case);
    assert_ne!(outcome.result.as_ref().unwrap_err(), "Operation aborted");
    assert_effects(case, &outcome);
}
