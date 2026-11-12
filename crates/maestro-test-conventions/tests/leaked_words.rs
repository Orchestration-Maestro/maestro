//! Library and product words stay out of identifiers, literals, comments, pages and tracked names, through the public check.
#![cfg(test)]

use crate::support::{Workspace, git, source};
use maestro_test_conventions::check_workspace;
use std::path::{Path, PathBuf};

/// Fill the placeholders `@1@` to `@5@` with the denied words, so this file never spells them.
///
/// `@2C@` and `@3C@` fill the capitalized forms of the second and third.
fn fill(text: &str) -> String {
    let words = [
        ("@2C@", ["Docx", "Preview"].concat()),
        ("@3C@", ["Mini", "match"].concat()),
        ("@1@", ["s", "sf"].concat()),
        ("@2@", ["docx", "_preview"].concat()),
        ("@3@", ["mini", "match"].concat()),
        ("@4@", ["plu", "gin"].concat()),
        ("@5@", ["lib", "ghostty"].concat()),
    ];
    words.iter().fold(text.to_owned(), |text, (mark, word)| {
        text.replace(mark, word)
    })
}

/// The note every diagnostic ends with.
const ADVICE: &str = "say it in Maestro's own words (words: DENIED and HASHED in leaked_words.rs)";

/// Write a tracked file below the fixture root, starting the index on first use.
fn track(workspace: &Workspace, relative: &str) {
    let root = workspace.root.path();
    if !root.join(".git").exists() {
        assert!(git(root).args(["init", "-q"]).status().unwrap().success());
    }
    let relative = fill(relative);
    let relative = relative.as_str();
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{}\n").unwrap();
    assert!(
        git(root)
            .args(["add", "--", relative])
            .status()
            .unwrap()
            .success()
    );
}

/// Write a tracked page with the given contents.
fn track_page(workspace: &Workspace, relative: &str, contents: &str) {
    track(workspace, relative);
    std::fs::write(workspace.root.path().join(fill(relative)), fill(contents)).unwrap();
}

/// The located identifier diagnostic.
fn identifier(path: &Path, line: usize, name: &str, word: &str) -> String {
    format!(
        "{}:{line}: identifier `{}` holds the word `{}`; {ADVICE}",
        path.display(),
        fill(name),
        fill(word)
    )
}

/// The located diagnostic for text of the given kind.
fn text(path: &Path, line: usize, kind: &str, word: &str) -> String {
    format!(
        "{}:{line}: {kind} holds the word `{}`; {ADVICE}",
        path.display(),
        fill(word)
    )
}

/// The diagnostic for a tracked file or directory name.
fn named(path: &str, name: &str, word: &str) -> String {
    format!(
        "{}: file or directory name `{}` holds the word `{}`; {ADVICE}",
        fill(path),
        fill(name),
        fill(word)
    )
}

/// A text member whose library holds the given source.
fn text_source(contents: &str) -> (Workspace, PathBuf) {
    let workspace = Workspace::with_text_member();
    let path = source(&workspace, "text", "src/lib.rs", &fill(contents));
    (workspace, path)
}

#[test]
fn a_denied_identifier_is_rejected_with_its_file_line_and_word() {
    let (workspace, path) = text_source(
        "//! Text.\nfn read() {}\nfn parse_@1@_value() {}\nstruct @2C@;\nfn @3C@() {}\n",
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([
            identifier(&path, 3, "parse_@1@_value", "@1@"),
            identifier(&path, 4, "@2C@", "@2@"),
            identifier(&path, 5, "@3C@", "@3@"),
        ]
        .join("\n"))
    );
}

#[test]
fn the_same_tree_passes_once_the_names_say_what_they_do() {
    let (workspace, _) = text_source(
        "//! Text.\nfn read() {}\nfn parse_format_value() {}\nstruct WordView;\nfn glob_matcher() {}\n",
    );

    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn longer_words_are_not_denied_words() {
    let (workspace, _) = text_source(
        "//! Text, read as successful @4@s.\nconst LABEL: &str = \"docx_body @4@s\";\nfn successful() {}\nfn @4@s() {}\nfn docx_body() {}\n",
    );

    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn a_denied_word_in_a_literal_or_comment_is_rejected_with_its_line() {
    let (workspace, path) = text_source(
        "//! Text, once read through @1@.\n// @4@\nconst LABEL: &str = \"a @2@\";\nconst BYTES: &[u8] = b\"\\x1bP>|@5@\\x1b\\\\\";\n/* @3@ */\n",
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([
            text(&path, 1, "doc comment", "@1@"),
            text(&path, 2, "comment", "@4@"),
            text(&path, 3, "string literal", "@2@"),
            text(&path, 4, "string literal", "@5@"),
            text(&path, 5, "comment", "@3@"),
        ]
        .join("\n"))
    );
}

#[test]
fn the_two_strings_that_reached_main_are_rejected() {
    let (workspace, path) = text_source(
        "//! Text.\npub const REPLY: &[u8] = b\"\\x1bP>|@5@\\x1b\\\\\";\nfn component() -> String {\n    \"%@4@\".to_string()\n}\n",
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([
            text(&path, 2, "string literal", "@5@"),
            text(&path, 4, "string literal", "@4@"),
        ]
        .join("\n"))
    );
}

#[test]
fn the_two_strings_as_reworded_pass() {
    let (workspace, _) = text_source(
        "//! Text.\npub const REPLY: &[u8] = b\"\\x1bP>|maestro\\x1b\\\\\";\nfn component() -> String {\n    \"%action\".to_string()\n}\n",
    );

    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn a_denied_word_in_a_documentation_page_is_rejected_with_its_line() {
    let workspace = Workspace::with_text_member();
    track_page(
        &workspace,
        "docs/tools.md",
        "# Tools\n\nRead it with @3@.\n",
    );
    track_page(&workspace, "docs/clean.md", "# Clean\n\nGlob matching.\n");

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "docs/tools.md:3: documentation page holds the word `{}`; {ADVICE}",
            fill("@3@")
        ))
    );
}

#[test]
fn an_untracked_page_and_a_page_outside_docs_are_not_read() {
    let workspace = Workspace::with_text_member();
    track_page(&workspace, "README.md", "@3@\n");
    track(&workspace, "docs/clean.md");
    std::fs::write(workspace.root.path().join("docs/draft.md"), fill("@3@\n")).unwrap();

    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn a_tracked_file_or_directory_name_with_a_denied_word_is_rejected() {
    let workspace = Workspace::with_text_member();
    track(&workspace, "docs/@1@-notes.md");
    track(&workspace, "crates/text/tests/fixtures/@1@/rows.json");

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([
            named("crates/text/tests/fixtures/@1@", "@1@", "@1@"),
            named("docs/@1@-notes.md", "@1@-notes.md", "@1@"),
        ]
        .join("\n"))
    );
}

#[test]
fn renamed_tracked_names_untracked_files_and_other_directories_pass() {
    let workspace = Workspace::with_text_member();
    track(&workspace, "docs/spreadsheet-notes.md");
    track(
        &workspace,
        "crates/text/tests/fixtures/spreadsheet/rows.json",
    );
    track(&workspace, "assets/@1@.json");
    std::fs::write(workspace.root.path().join(fill("docs/@1@-draft.md")), "").unwrap();

    check_workspace(workspace.root.path()).unwrap();
}
