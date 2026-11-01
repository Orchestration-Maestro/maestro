//! Original-file matching and atomic replacement through the application tool factory.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::{
    run_io,
    session_harness::{as_recorded, case},
};
use maestro_app::{EditOperations, EditToolOptions};
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use serde_json::{Value, json};
use std::{fmt::Write as _, sync::Arc};

/// Runs one edit against a disposable file and compares both its result and final bytes.
fn check_edit(name: &str, before: &str, edits: Value) {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join("file.txt");
    std::fs::write(&path, before).unwrap();
    let tool = maestro_app::create_edit_tool(
        root.path().display().to_string(),
        EditToolOptions::default(),
    );
    let mut arguments = json!({"path":path.display().to_string()});
    arguments["edits"] = edits;
    let result = run_io((tool.execute)("edit".into(), arguments, None, None));
    let mut observed = match result {
        Ok(result) => json!({"ok":result}),
        Err(error) => json!({"error":error.message}),
    };
    observed["after"] = json!({"content":std::fs::read_to_string(path).unwrap()});
    assert_eq!(
        as_recorded(observed, &[(&root.path().display().to_string(), "$ROOT")]),
        case("tools_root", name)["expected"]
    );
}

#[test]
fn edit_returns_replacement_and_numbered_diff() {
    check_edit(
        "edit_returns_replacement_and_numbered_diff",
        "Hello, world!",
        json!([{"oldText":"world","newText":"testing"}]),
    );
}

#[test]
fn edit_rejects_absent_old_text() {
    check_edit(
        "edit_rejects_absent_old_text",
        "Hello, world!",
        json!([{"oldText":"nonexistent","newText":"testing"}]),
    );
}

#[test]
fn edit_preserves_missing_file_failure_kind() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let native = std::fs::read(root.path().join("missing.txt")).unwrap_err();
    assert_eq!(native.kind(), std::io::ErrorKind::NotFound);
    let tool = maestro_app::create_edit_tool(
        root.path().display().to_string(),
        EditToolOptions::default(),
    );
    let error = run_io((tool.execute)(
        "edit".into(),
        json!({"path":"missing.txt","edits":[{"oldText":"hello","newText":"world"}]}),
        None,
        None,
    ))
    .unwrap_err();
    assert!(error.message.contains("missing.txt"), "{error:?}");
    assert!(
        error
            .message
            .contains(&format!("Error code: {}.", native.raw_os_error().unwrap())),
        "{error:?}"
    );
    assert!(!root.path().join("missing.txt").exists());
}

#[test]
fn edit_rejects_multiple_exact_occurrences() {
    check_edit(
        "edit_rejects_multiple_exact_occurrences",
        "foo foo foo",
        json!([{"oldText":"foo","newText":"bar"}]),
    );
}

#[test]
fn edit_applies_disjoint_blocks_together() {
    check_edit(
        "edit_applies_disjoint_blocks_together",
        "alpha\nbeta\ngamma\ndelta\n",
        json!([{"oldText":"alpha\n","newText":"ALPHA\n"},{"oldText":"gamma\n","newText":"GAMMA\n"}]),
    );
}

#[test]
fn edit_collapses_distant_unchanged_hunks() {
    let mut before = String::new();
    for line in 1..=600 {
        writeln!(before, "line {line:03}").unwrap();
    }
    check_edit(
        "edit_collapses_distant_unchanged_hunks",
        &before,
        json!([{"oldText":"line 100\n","newText":"LINE 100\n"},{"oldText":"line 300\n","newText":"LINE 300\n"},{"oldText":"line 500\n","newText":"LINE 500\n"}]),
    );
}

#[test]
fn edit_matches_each_block_against_original() {
    check_edit(
        "edit_matches_each_block_against_original",
        "foo\nbar\nbaz\n",
        json!([{"oldText":"foo\n","newText":"foo bar\n"},{"oldText":"bar\n","newText":"BAR\n"}]),
    );
}

#[test]
fn edit_rejects_empty_replacement_list() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let file = root.path().join("file.txt");
    std::fs::write(&file, "hello\nworld\n").unwrap();
    let tool = maestro_app::create_edit_tool(
        root.path().display().to_string(),
        EditToolOptions::default(),
    );
    let error = run_io((tool.execute)(
        "edit".into(),
        json!({"path":"file.txt","edits":[]}),
        None,
        None,
    ))
    .unwrap_err();
    assert!(
        error.message.contains("edits") && error.message.contains("at least one"),
        "{error:?}"
    );
    assert_eq!(std::fs::read(file).unwrap(), b"hello\nworld\n");
}

#[test]
fn edit_rejects_overlapping_regions() {
    check_edit(
        "edit_rejects_overlapping_regions",
        "one\ntwo\nthree\n",
        json!([{"oldText":"one\ntwo\n","newText":"ONE\nTWO\n"},{"oldText":"two\nthree\n","newText":"TWO\nTHREE\n"}]),
    );
}

#[test]
fn edit_leaves_file_unchanged_after_later_failure() {
    check_edit(
        "edit_leaves_file_unchanged_after_later_failure",
        "alpha\nbeta\ngamma\n",
        json!([{"oldText":"alpha\n","newText":"ALPHA\n"},{"oldText":"missing\n","newText":"MISSING\n"}]),
    );
}

#[cfg(unix)]
#[test]
fn edit_preserves_write_access_failure_kind() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let file = root.path().join("file.txt");
    std::fs::write(&file, "hello\n").unwrap();
    let Some(_denied) = maestro_test_directory::deny(&file, 0o444).unwrap() else {
        eprintln!("host permits writes despite denied file mode");
        return;
    };
    let tool = maestro_app::create_edit_tool(
        root.path().display().to_string(),
        EditToolOptions::default(),
    );
    let native = std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap_err();
    assert_eq!(native.kind(), std::io::ErrorKind::PermissionDenied);
    let error = run_io((tool.execute)(
        "edit".into(),
        json!({"path":"file.txt","edits":[{"oldText":"hello","newText":"world"}]}),
        None,
        None,
    ))
    .unwrap_err();
    assert!(
        error
            .message
            .contains(&format!("Error code: {}.", native.raw_os_error().unwrap())),
        "{error:?}"
    );
    assert_eq!(std::fs::read(file).unwrap(), b"hello\n");
}

/// An access failure that must prevent reads and writes.
struct UnavailableEdit;

impl EditOperations for UnavailableEdit {
    fn access(&self, _: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Err(DiagnosticErrorInfo::error(
            "disk offline",
        ))))
    }
    fn read_file(&self, _: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        panic!("access failure must prevent reading")
    }
    fn write_file(&self, _: String, _: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        panic!("access failure must prevent writing")
    }
}

#[test]
fn edit_preserves_supplied_access_failure() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let tool = maestro_app::create_edit_tool(
        root.path().display().to_string(),
        EditToolOptions {
            operations: Some(Arc::new(UnavailableEdit)),
        },
    );
    let error = run_io((tool.execute)(
        "edit".into(),
        json!({"path":"broken.txt","edits":[{"oldText":"a","newText":"b"}]}),
        None,
        None,
    ))
    .unwrap_err();
    assert!(
        error.message.contains("disk offline") && error.message.contains("broken.txt"),
        "{error:?}"
    );
    assert!(!root.path().join("broken.txt").exists());
}

#[test]
fn edit_fuzzy_match_trims_trailing_whitespace() {
    check_edit(
        "edit_fuzzy_match_trims_trailing_whitespace",
        "line one   \nline two  \nline three\n",
        json!([{"oldText":"line one\nline two\n","newText":"replaced\n"}]),
    );
    let boundary = json!([{"oldText":"alpha\nbeta\n","newText":"changed\n"}]);
    check_edit(
        "edit_fuzzy_match_trims_trailing_whitespace/feff",
        "alpha\u{feff}\nbeta\n",
        boundary.clone(),
    );
    check_edit(
        "edit_fuzzy_match_trims_trailing_whitespace/0085",
        "alpha\u{85}\nbeta\n",
        boundary,
    );
}

#[test]
fn edit_fuzzy_match_accepts_fullwidth_punctuation() {
    check_edit(
        "edit_fuzzy_match_accepts_fullwidth_punctuation",
        "你好，世界\n你好（世界）\n",
        json!([{"oldText":"你好,世界\n你好(世界)\n","newText":"你好，maestro\n你好(maestro)\n"}]),
    );
}

#[test]
fn edit_fuzzy_match_accepts_compatibility_forms() {
    check_edit(
        "edit_fuzzy_match_accepts_compatibility_forms",
        "ＡＢＣ１２３\ncafe\u{301}\n",
        json!([{"oldText":"ABC123\ncafé\n","newText":"XYZ789\ncoffee\n"}]),
    );
}

#[test]
fn edit_fuzzy_match_accepts_curly_single_quotes() {
    check_edit(
        "edit_fuzzy_match_accepts_curly_single_quotes",
        "console.log(‘hello’);\n",
        json!([{"oldText":"console.log('hello');","newText":"console.log('world');"}]),
    );
}

#[test]
fn edit_fuzzy_match_accepts_curly_double_quotes() {
    check_edit(
        "edit_fuzzy_match_accepts_curly_double_quotes",
        "const msg = “Hello World”;\n",
        json!([{"oldText":"const msg = \"Hello World\";","newText":"const msg = \"Goodbye\";"}]),
    );
}

#[test]
fn edit_fuzzy_match_accepts_unicode_dashes() {
    check_edit(
        "edit_fuzzy_match_accepts_unicode_dashes",
        "range: 1–5\nbreak—here\n",
        json!([{"oldText":"range: 1-5\nbreak-here","newText":"range: 10-50\nbreak--here"}]),
    );
}

#[test]
fn edit_fuzzy_match_accepts_nonbreaking_spaces() {
    check_edit(
        "edit_fuzzy_match_accepts_nonbreaking_spaces",
        "hello\u{a0}world\n",
        json!([{"oldText":"hello world","newText":"hello universe"}]),
    );
}

#[test]
fn edit_prefers_available_exact_match() {
    check_edit(
        "edit_prefers_available_exact_match",
        "const x = 'exact';\nconst y = ‘other’;\n",
        json!([{"oldText":"const x = 'exact';","newText":"const x = 'changed';"}]),
    );
}

#[test]
fn edit_fuzzy_match_still_rejects_unrelated_text() {
    check_edit(
        "edit_fuzzy_match_still_rejects_unrelated_text",
        "completely different content\n",
        json!([{"oldText":"this does not exist","newText":"replacement"}]),
    );
}

#[test]
fn edit_fuzzy_match_rejects_normalized_duplicates() {
    check_edit(
        "edit_fuzzy_match_rejects_normalized_duplicates",
        "hello world   \nhello world\n",
        json!([{"oldText":"hello world \n","newText":"replaced"}]),
    );
}

#[test]
fn edit_multi_block_fuzzy_match_preserves_each_replacement() {
    check_edit(
        "edit_multi_block_fuzzy_match_preserves_each_replacement",
        "console.log(‘hello’);\nhello\u{a0}world\n",
        json!([{"oldText":"console.log('hello');\n","newText":"console.log('world');\n"},{"oldText":"hello world\n","newText":"hello universe\n"}]),
    );
}

#[test]
fn edit_accepts_lf_match_inside_crlf_file() {
    check_edit(
        "edit_accepts_lf_match_inside_crlf_file",
        "line one\r\nline two\r\nline three\r\n",
        json!([{"oldText":"line two\n","newText":"replaced line\n"}]),
    );
}

#[test]
fn edit_restores_crlf_after_replacement() {
    check_edit(
        "edit_restores_crlf_after_replacement",
        "first\r\nsecond\r\nthird\r\n",
        json!([{"oldText":"second\n","newText":"REPLACED\n"}]),
    );
}

#[test]
fn edit_keeps_lf_after_replacement() {
    check_edit(
        "edit_keeps_lf_after_replacement",
        "first\nsecond\nthird\n",
        json!([{"oldText":"second\n","newText":"REPLACED\n"}]),
    );
}

#[test]
fn edit_rejects_duplicates_across_line_endings() {
    check_edit(
        "edit_rejects_duplicates_across_line_endings",
        "hello\r\nworld\r\n---\r\nhello\nworld\n",
        json!([{"oldText":"hello\nworld\n","newText":"replaced\n"}]),
    );
}

#[test]
fn edit_retains_utf8_bom() {
    check_edit(
        "edit_retains_utf8_bom",
        "\u{feff}first\r\nsecond\r\nthird\r\n",
        json!([{"oldText":"second\n","newText":"REPLACED\n"}]),
    );
}

#[test]
fn edit_multi_block_retains_bom_and_crlf() {
    check_edit(
        "edit_multi_block_retains_bom_and_crlf",
        "\u{feff}first\r\nsecond\r\nthird\r\nfourth\r\n",
        json!([{"oldText":"second\n","newText":"SECOND\n"},{"oldText":"fourth\n","newText":"FOURTH\n"}]),
    );
}

#[test]
fn edit_root_preparation_retains_in_place_and_returned_forms() {
    let cases = [
        (
            "array",
            json!({"path":"file.txt","edits":[{"oldText":"a","newText":"A"}]}),
        ),
        (
            "serialized",
            json!({"path":"file.txt","edits":"[{\"oldText\":\"a\",\"newText\":\"A\"}]"}),
        ),
        (
            "legacy",
            json!({"path":"file.txt","oldText":"a","newText":"A"}),
        ),
        (
            "mixed",
            json!({"path":"file.txt","edits":[{"oldText":"a","newText":"A"}],"oldText":"b","newText":"B"}),
        ),
        ("malformed", json!({"path":"file.txt","edits":"{bad"})),
        ("nonarray", json!({"path":"file.txt","edits":"{}"})),
        ("empty", json!({"path":"file.txt","edits":[]})),
    ];
    for (name, mut retained) in cases {
        let root = maestro_test_directory::Scratch::new().unwrap();
        let file = root.path().join("file.txt");
        std::fs::write(&file, "a b").unwrap();
        let tool = maestro_app::create_edit_tool(
            root.path().display().to_string(),
            EditToolOptions::default(),
        );
        let prepared = (tool.prepare_arguments.as_ref().unwrap())(&mut retained).unwrap();
        let result = run_io((tool.execute)("edit".into(), prepared.clone(), None, None));
        let mut observed = json!({"ok":{"retained":retained,"prepared":prepared},"after":{"content":std::fs::read_to_string(file).unwrap()}});
        match result {
            Ok(result) => observed["ok"]["result"] = json!(result),
            Err(error) => {
                assert!(
                    error.message.contains("invalid type")
                        || error.message.contains("at least one"),
                    "{error:?}"
                );
                observed["ok"]["failureKind"] = json!("empty-or-invalid-edits");
            }
        }
        assert_eq!(
            observed,
            case(
                "tools_root",
                &format!("edit_root_preparation_retains_in_place_and_returned_forms/{name}")
            )["expected"]
        );
    }
}
