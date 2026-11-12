//! Library and product words stay out of identifiers and tracked names, through the public check.
#![cfg(test)]

use crate::support::{Workspace, git, source};
use maestro_test_conventions::check_workspace;
use std::path::{Path, PathBuf};

/// The note every diagnostic ends with.
const ADVICE: &str = "name it for what it does (words: DENIED in leaked_words.rs)";

/// Write a tracked file below the fixture root, starting the index on first use.
fn track(workspace: &Workspace, relative: &str) {
    let root = workspace.root.path();
    if !root.join(".git").exists() {
        assert!(git(root).args(["init", "-q"]).status().unwrap().success());
    }
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

/// The located identifier diagnostic.
fn identifier(path: &Path, line: usize, name: &str, word: &str) -> String {
    format!(
        "{}:{line}: identifier `{name}` holds the word `{word}`; {ADVICE}",
        path.display()
    )
}

/// The diagnostic for a tracked file or directory name.
fn named(path: &str, name: &str, word: &str) -> String {
    format!("{path}: file or directory name `{name}` holds the word `{word}`; {ADVICE}")
}

/// A text member whose library holds the given source.
fn text_source(contents: &str) -> (Workspace, PathBuf) {
    let workspace = Workspace::with_text_member();
    let path = source(&workspace, "text", "src/lib.rs", contents);
    (workspace, path)
}

#[test]
fn a_denied_identifier_is_rejected_with_its_file_line_and_word() {
    let (workspace, path) = text_source(
        "//! Text.\nfn read() {}\nfn parse_ssf_value() {}\nstruct DocxPreview;\nfn Minimatch() {}\n",
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([
            identifier(&path, 3, "parse_ssf_value", "ssf"),
            identifier(&path, 4, "DocxPreview", "docx_preview"),
            identifier(&path, 5, "Minimatch", "minimatch"),
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
fn literals_comments_and_longer_words_are_not_names() {
    let (workspace, _) = text_source(
        "//! Text, once read through ssf and minimatch.\n// plugin\nconst LABEL: &str = \"ssf docx_preview\";\nfn successful() {}\nfn plugins() {}\nfn docx_body() {}\n",
    );

    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn a_tracked_file_or_directory_name_with_a_denied_word_is_rejected() {
    let workspace = Workspace::with_text_member();
    track(&workspace, "docs/ssf-notes.md");
    track(&workspace, "crates/text/tests/fixtures/ssf/rows.json");

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err([
            named("crates/text/tests/fixtures/ssf", "ssf", "ssf"),
            named("docs/ssf-notes.md", "ssf-notes.md", "ssf"),
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
    track(&workspace, "assets/ssf.json");
    std::fs::write(workspace.root.path().join("docs/ssf-draft.md"), "").unwrap();

    check_workspace(workspace.root.path()).unwrap();
}
