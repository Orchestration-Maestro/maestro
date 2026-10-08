#![cfg(test)]

mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

#[test]
fn block_documentation_is_rejected_with_file_and_line() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    let path = workspace.root.join("crates/models/src/lib.rs");
    for source in ["\n/** Item. */\npub struct Item;\n", "\n/*! Crate. */\n"] {
        std::fs::write(&path, source).unwrap();
        let error = check_workspace(&workspace.root).unwrap_err();
        assert_eq!(
            error,
            format!("{}:2: block documentation: use /// or //!", path.display())
        );
    }
}

#[test]
fn only_generated_guest_bindings_allow_block_documentation() {
    let workspace = Workspace::new();
    workspace.member("guest", "maestro-extensions-wasm", "");
    workspace.list(&[("maestro-extensions-wasm", "core")]);
    let root = workspace.root.join("crates/guest/src");
    let bindings = root.join("bindings.rs");
    std::fs::write(
        &bindings,
        "//! Crate.\n/*! Crate. */\n//! Crate.\n/// Item.\n/** Item. */\n/// Item.\npub struct Item;",
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    let manual = root.join("manual.rs");
    std::fs::write(&manual, "/** Item. */\npub struct Item;").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert_eq!(
        error,
        format!(
            "{}:1: block documentation: use /// or //!",
            manual.display()
        )
    );
    std::fs::remove_file(manual).unwrap();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[
        ("maestro-extensions-wasm", "core"),
        ("maestro-models", "core"),
    ]);
    let other = workspace.root.join("crates/models/src/bindings.rs");
    std::fs::write(&other, "/*! Crate. */").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert_eq!(
        error,
        format!("{}:1: block documentation: use /// or //!", other.display())
    );
}

#[test]
fn numbered_planning_comments_are_rejected_with_file_and_line() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    let path = workspace.root.join("crates/models/src/lib.rs");
    for (contents, line) in [
        ("\n// slice 6\n", 2),
        ("\r\n// café 🦀 SLICE 6\r\n", 2),
        ("/* issue 1\n Unicode 🦀 #23 */", 2),
        ("\u{feff}#!/usr/bin/env rustx\n// café ticket 1\n", 2),
    ] {
        std::fs::write(&path, contents).unwrap();
        let error = check_workspace(&workspace.root).unwrap_err();
        assert_location(&error, &path, line);
        std::fs::write(&path, "// Technical comment.\n").unwrap();
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn crlf_and_lf_sources_report_identical_comment_lines() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[("maestro-models", "core")]);
    let path = workspace.root.join("crates/models/src/lib.rs");
    let lf = "\u{feff}#!/usr/bin/env rustx\nconst TEXT: &str = r#\"first\nsecond\"#;\n\n// café ticket 1\n";
    std::fs::write(&path, lf).unwrap();
    let expected = check_workspace(&workspace.root).unwrap_err();
    assert_location(&expected, &path, 5);
    std::fs::write(&path, lf.replace('\n', "\r\n")).unwrap();
    assert_eq!(check_workspace(&workspace.root), Err(expected));
    std::fs::write(&path, lf.replace("ticket 1", "Technical")).unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
    for noun in ["slice", "spec", "task", "ticket", "issue", "pr"] {
        rejects(&[
            &format!("// {noun} 6"),
            &format!("/* {} 0 */", noun.to_uppercase()),
        ]);
    }
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
        r###"
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
// spec 1a; slice １２; user stories; sprinting; milestones; us01; AT04; D07x.
"###,
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
    std::fs::write(&path, "/* Outer /* technical */ comment. */").unwrap();
    for excluded in ["target", ".git"] {
        let ignored = workspace.root.join("crates/models").join(excluded);
        std::fs::create_dir(&ignored).unwrap();
        std::fs::write(ignored.join("ignored.rs"), "// ticket 5").unwrap();
    }
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}
