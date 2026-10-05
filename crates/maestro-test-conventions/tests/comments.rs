mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

#[test]
fn numbered_planning_comments_are_rejected_with_file_and_line() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    let path = workspace.root.join("crates/models/src/lib.rs");
    std::fs::write(&path, "\n// slice 6\n").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert_location(&error, &path, 2);
}

fn assert_location(error: &str, expected: &std::path::Path, line: usize) {
    let suffix = format!(":{line}: planning reference in comment");
    let reported = error.strip_suffix(&suffix).expect(error);
    assert_eq!(
        std::path::Path::new(reported).canonicalize().unwrap(),
        expected.canonicalize().unwrap(),
        "{error}"
    );
}

fn rejects(comments: &[&str]) {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    let path = workspace.root.join("crates/models/src/lib.rs");
    for comment in comments {
        std::fs::write(&path, format!("\n{comment}\n")).unwrap();
        let error = check_workspace(&workspace.root).unwrap_err();
        assert_location(&error, &path, 2);
    }
    std::fs::write(&path, "// Describes model behavior.\n").unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn all_numbered_nouns_are_rejected() {
    rejects(&[
        "// slice 6",
        "/// spec 1 task 2",
        "//! task 2",
        "/* ticket 5 */",
        "// issue 18",
        "// SLICE 6",
    ]);
}

#[test]
fn hash_number_references_are_rejected() {
    rejects(&["// #18", "/// issue #18"]);
}

#[test]
fn numbered_pull_requests_are_rejected() {
    rejects(&["// pull request 17", "// PR 17"]);
}

#[test]
fn planning_vocabulary_is_rejected() {
    rejects(&["// milestone", "// sprint", "/* user story */"]);
}

#[test]
fn planning_identifiers_are_rejected() {
    rejects(&["// US01", "// FR-S2-024", "// D07", "// F48", "// T04"]);
}

#[test]
fn technical_comments_and_planning_text_outside_comments_pass() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    std::fs::write(
        workspace.root.join("crates/models/src/lib.rs"),
        r####"
//! the JSON-RPC specification; an async task; step 1 of the parse.
/// phase one writes the intent; UTF-16; S3-compatible; x86_64; 0x1F.
/* Outer /* technical */ nested comment. */
#[allow(dead_code)]
const TEXT: &str = "// slice 6 \" /* #18 */";
const RAW: &str = r##"/* ticket 5 */ \"#18"##;
const BYTES: &[u8] = br#"// PR 17"#;
const CHARACTER: char = '"';
const ESCAPED: char = '\'';
fn borrow<'a>(value: &'a str) -> &'a str { value }
// Technical Unicode: café 🦀.
"####,
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn nested_block_comments_report_the_inner_line_in_non_src_files() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    let path = workspace.root.join("crates/models/build.rs");
    std::fs::write(&path, "/* technical\n /* ticket 5 */\n*/\n").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert_location(&error, &path, 2);
}
