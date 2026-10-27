//! Read-only edit previews through the public operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::read_support;
use crate::runtime::run;
use maestro_tools::edit_diff::{Edit, EditDiffResult, compute_edit_diff, compute_edits_diff};
use read_support::{Fixture, Scratch, isolated};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
};

/// The closed native-preview corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    tests: BTreeMap<String, Vec<Case>>,
}
/// A target, its setup and the recorded public result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    file: String,
    path: Option<String>,
    content: Option<String>,
    bytes: Option<Vec<u8>>,
    setup: Option<String>,
    edits: Vec<Replacement>,
    single: Option<bool>,
    expected: Expected,
}
/// Authored replacement operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Replacement {
    old_text: String,
    new_text: String,
}
/// The recorded success or classified native failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    diff: Option<String>,
    first_changed_line: Option<usize>,
    error: Option<String>,
    access_code: Option<String>,
    read_code: Option<String>,
}
/// Create a file exclusively so a setup mistake cannot overwrite another target.
fn create(path: &std::path::Path, bytes: &[u8]) {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}
/// Materialize a case below the scratch root.
fn setup(root: &Scratch, case: &Case) -> Option<Vec<u8>> {
    let file = root.path().join(&case.file);
    assert!(!(case.content.is_some() && case.bytes.is_some()));
    let bytes = case
        .bytes
        .clone()
        .or_else(|| case.content.as_ref().map(|text| text.as_bytes().to_vec()));
    if let Some(bytes) = &bytes {
        create(&file, bytes);
    }
    match case.setup.as_deref() {
        None => {}
        Some("directory") => fs::create_dir(&file).unwrap(),
        Some("parentFile") => create(file.parent().unwrap(), b"parent"),
        Some("decomposedSibling") => create(&root.path().join("cafe\u{301}.txt"), b"hello\n"),
        #[cfg(unix)]
        Some("readonly" | "unreadable") => {
            use std::os::unix::fs::PermissionsExt;
            let mode = if case.setup.as_deref() == Some("readonly") {
                0o444
            } else {
                0
            };
            fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
        }
        #[cfg(unix)]
        Some("selfLink") => std::os::unix::fs::symlink(&case.file, &file).unwrap(),
        Some(other) => panic!("unknown setup {other}"),
    }
    bytes
}
/// Independently observe the native access error and its declared class.
#[cfg(unix)]
fn access_error(path: &std::path::Path, code: &str) -> io::Error {
    use rustix::io::Errno;
    let expected = match code {
        "ENOENT" => Errno::NOENT,
        "EACCES" => Errno::ACCESS,
        "ENOTDIR" => Errno::NOTDIR,
        "ELOOP" => Errno::LOOP,
        "ENAMETOOLONG" => Errno::NAMETOOLONG,
        _ => panic!("unknown access class"),
    };
    let error: io::Error = rustix::fs::access(path, rustix::fs::Access::READ_OK)
        .unwrap_err()
        .into();
    assert_eq!(error.raw_os_error(), Some(expected.raw_os_error()));
    assert_eq!(
        error.kind(),
        io::Error::from_raw_os_error(expected.raw_os_error()).kind()
    );
    error
}
/// Whether a privileged run ignores the case's denied modes, leaving nothing to check.
#[cfg(unix)]
fn denial_ignored(root: &Scratch, case: &Case) -> bool {
    matches!(case.setup.as_deref(), Some("readonly" | "unreadable"))
        && maestro_test_directory::deny(root.text(), 0o0)
            .unwrap()
            .is_none()
}
/// Whether the host folds the case's decomposed sibling onto the case's own spelling: it then
/// finds the sibling under that spelling, so no fallback would be needed to reach it.
fn sibling_folded(root: &Scratch, case: &Case) -> bool {
    let pair = [case.file.as_str(), "cafe\u{301}.txt"].map(std::ffi::OsStr::new);
    let folded =
        case.setup.as_deref() == Some("decomposedSibling") && !root.keeps_distinct(&pair).unwrap();
    if folded {
        eprintln!("the sibling case needs a host that keeps {pair:?} apart");
    }
    folded
}
/// Compare the public result against one recorded native case.
fn check_case(case: &Case) {
    let root = Scratch::new().unwrap();
    #[cfg(unix)]
    if denial_ignored(&root, case) {
        return;
    }
    if sibling_folded(&root, case) {
        return;
    }
    let bytes = setup(&root, case);
    let path = case.path.as_ref().map_or_else(
        || case.file.clone(),
        |path| path.replace("$ROOT", &root.text()),
    );
    let edits = case
        .edits
        .iter()
        .map(|edit| Edit {
            old_text: edit.old_text.clone(),
            new_text: edit.new_text.clone(),
        })
        .collect();
    let future = if case.single.unwrap_or(false) {
        assert_eq!(case.edits.len(), 1);
        compute_edit_diff(
            &path,
            &case.edits[0].old_text,
            &case.edits[0].new_text,
            &root.text(),
        )
    } else {
        compute_edits_diff(&path, edits, &root.text())
    };
    let result = run(future.unwrap());
    let expected = &case.expected;
    assert_eq!(
        [
            expected.diff.is_some(),
            expected.error.is_some(),
            expected.access_code.is_some(),
            expected.read_code.is_some()
        ]
        .into_iter()
        .filter(|present| *present)
        .count(),
        1
    );
    if let Some(diff) = &expected.diff {
        assert_eq!(
            result.unwrap(),
            EditDiffResult {
                diff: diff.clone(),
                first_changed_line: expected.first_changed_line
            }
        );
    } else if let Some(error) = &expected.error {
        assert_eq!(result.unwrap_err().error, error.replace(&case.file, &path));
    } else {
        check_native_failure(&root, case, &path, &result.unwrap_err().error);
    }
    assert_retained(&root, case, bytes);
}
/// Read the same target after completion to prove no stored bytes changed.
fn assert_retained(root: &Scratch, case: &Case, bytes: Option<Vec<u8>>) {
    if let Some(bytes) = bytes {
        #[cfg(unix)]
        if case.setup.as_deref() == Some("unreadable") {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                root.path().join(&case.file),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        assert_eq!(fs::read(root.path().join(&case.file)).unwrap(), bytes);
    }
}
/// Native failures retain their phase and native diagnostics.
#[cfg(unix)]
fn check_native_failure(root: &Scratch, case: &Case, path: &str, actual: &str) {
    let resolved = maestro_tools::path_utils::resolve_to_cwd(path, &root.text()).unwrap();
    if let Some(code) = &case.expected.access_code {
        let native = access_error(std::path::Path::new(&resolved), code);
        assert_eq!(
            actual,
            format!(
                "Could not edit file: {path}. Error code: {}.",
                native.raw_os_error().unwrap()
            )
        );
    } else {
        assert_eq!(case.expected.read_code.as_deref(), Some("EISDIR"));
        rustix::fs::access(&resolved, rustix::fs::Access::READ_OK).unwrap();
        let native = fs::read(resolved).unwrap_err();
        assert_eq!(native.kind(), io::ErrorKind::IsADirectory);
        assert_eq!(actual, native.to_string());
    }
}
/// Native classification rows run only on Unix.
#[cfg(not(unix))]
fn check_native_failure(_: &Scratch, _: &Case, _: &str, _: &str) {
    panic!("Unix fixture invoked on another platform");
}
/// Assert a nonempty group, with no unrecognized group or repeated invocation.
fn check(name: &str) {
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/edit-preview-cases.json")).unwrap();
    assert_eq!(
        corpus.tests.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "preview_accepts_readable_nonwritable_files",
            "preview_applies_disjoint_edits_to_original_snapshot",
            "preview_decodes_invalid_utf8_lossily",
            "preview_does_not_try_read_tool_filename_fallbacks",
            "preview_inherits_corrected_matching",
            "preview_large_edits_keep_numbered_hunks",
            "preview_normalizes_bom_newlines_without_writing",
            "preview_path_expansion_preserves_authored_error_path",
            "preview_preserves_match_failures_and_precedence",
            "preview_reports_native_access_failures",
            "preview_reports_read_failure_after_successful_access",
            "preview_uses_selected_fuzzy_base",
            "single_preview_matches_the_multi_edit_entry"
        ]
    );
    let mut queries = HashSet::new();
    for cases in corpus.tests.values() {
        assert!(!cases.is_empty());
        for case in cases {
            let edits: Vec<_> = case
                .edits
                .iter()
                .map(|edit| (&edit.old_text, &edit.new_text))
                .collect();
            assert!(queries.insert((&case.file, &case.path, edits, case.single)));
        }
    }
    assert_eq!(queries.len(), 33);
    let cases = corpus.tests.get(name).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        check_case(case);
    }
}

#[test]
fn preview_normalizes_bom_newlines_without_writing() {
    check("preview_normalizes_bom_newlines_without_writing");
}

#[test]
fn preview_uses_selected_fuzzy_base() {
    check("preview_uses_selected_fuzzy_base");
}

#[test]
fn preview_applies_disjoint_edits_to_original_snapshot() {
    check("preview_applies_disjoint_edits_to_original_snapshot");
}

#[test]
fn single_preview_matches_the_multi_edit_entry() {
    check("single_preview_matches_the_multi_edit_entry");
}

#[cfg(unix)]
#[test]
fn preview_accepts_readable_nonwritable_files() {
    check("preview_accepts_readable_nonwritable_files");
}

#[test]
fn preview_decodes_invalid_utf8_lossily() {
    check("preview_decodes_invalid_utf8_lossily");
}

#[cfg(unix)]
#[test]
fn preview_path_expansion_preserves_authored_error_path() {
    check("preview_path_expansion_preserves_authored_error_path");
}

#[cfg(unix)]
#[test]
fn preview_does_not_try_read_tool_filename_fallbacks() {
    check("preview_does_not_try_read_tool_filename_fallbacks");
}

#[test]
fn preview_preserves_match_failures_and_precedence() {
    check("preview_preserves_match_failures_and_precedence");
}

#[cfg(unix)]
#[test]
fn preview_reports_native_access_failures() {
    check("preview_reports_native_access_failures");
}

#[cfg(unix)]
#[test]
fn preview_reports_read_failure_after_successful_access() {
    check("preview_reports_read_failure_after_successful_access");
}

#[test]
fn preview_large_edits_keep_numbered_hunks() {
    check("preview_large_edits_keep_numbered_hunks");
}

#[test]
fn preview_inherits_corrected_matching() {
    check("preview_inherits_corrected_matching");
}

/// The same recorded result is required from both entry routes.
fn binding_preview(
    single: bool,
    path: &str,
    cwd: &str,
) -> io::Result<
    maestro_models::BoxFuture<Result<EditDiffResult, maestro_tools::edit_diff::EditDiffError>>,
> {
    if single {
        compute_edit_diff(path, "right", "RIGHT", cwd)
    } else {
        compute_edits_diff(
            path,
            vec![Edit {
                old_text: "right".into(),
                new_text: "RIGHT".into(),
            }],
            cwd,
        )
    }
}
/// Assert the recorded original-file result, not merely agreement between routes.
fn assert_original(result: &EditDiffResult) {
    assert_eq!(result.diff, " 1 left\n-2 right\n+2 RIGHT");
    assert_eq!(result.first_changed_line, Some(2));
}
#[test]
fn preview_binds_relative_cwd_before_polling() {
    if !isolated("preview_binds_relative_cwd_before_polling", "binding", &[]) {
        return;
    }
    let saved = std::env::current_dir().unwrap();
    let original = Scratch::new().unwrap();
    let decoy = Scratch::new().unwrap();
    original.file("target.txt", b"left\nright\n");
    decoy.file("target.txt", b"left\nright\nwrong\n");
    for single in [false, true] {
        std::env::set_current_dir(original.path()).unwrap();
        let future = binding_preview(single, "target.txt", ".").unwrap();
        std::env::set_current_dir(decoy.path()).unwrap();
        assert_original(&run(future).unwrap());
    }
    std::env::set_current_dir(saved).unwrap();
}
#[cfg(unix)]
#[test]
fn preview_reads_ambient_cwd_only_when_required() {
    if !isolated(
        "preview_reads_ambient_cwd_only_when_required",
        "ambient",
        &[],
    ) {
        return;
    }
    let saved = std::env::current_dir().unwrap();
    let original = Scratch::new().unwrap();
    let removed = maestro_test_directory::Scratch::new().unwrap();
    let target = original.file("target.txt", b"left\nright\n");
    removed.enter_removed("removed").unwrap();
    for single in [false, true] {
        assert_original(&run(binding_preview(single, &target, ".").unwrap()).unwrap());
        assert_original(
            &run(binding_preview(single, "target.txt", &original.text()).unwrap()).unwrap(),
        );
        let Err(error) = binding_preview(single, "target.txt", ".") else {
            panic!("resolution must fail before future construction")
        };
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
    std::env::set_current_dir(saved).unwrap();
}
