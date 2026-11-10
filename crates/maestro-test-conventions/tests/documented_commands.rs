//! Home-directory program paths in documented commands.
#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;
use std::path::{Path, PathBuf};

/// A workspace with one member and a page at `path` holding `contents`.
fn workspace(path: &str, contents: &str) -> (Workspace, PathBuf) {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    let page = workspace.root.path().join(path);
    std::fs::create_dir_all(page.parent().unwrap_or_else(|| Path::new(""))).unwrap();
    std::fs::write(&page, contents).unwrap();
    (workspace, page)
}

/// Run one git command in `root`, failing the test when it refuses.
fn run_git(root: &Path, arguments: &[&str]) {
    let output = crate::support::git(root).args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The diagnostic for a home-directory program at `line` of `page`.
fn offender(page: &Path, line: usize) -> String {
    format!(
        "{}:{line}: documented commands run cargo or just, not a program from a home directory",
        page.display()
    )
}

#[test]
fn a_documentation_fixture_naming_a_home_directory_program_is_reported() {
    let (workspace, page) = workspace(
        "crates/text/tests/fixtures/guide.md",
        "# Build\n\n```sh\n~/.local/bin/capped cargo build --locked\n```\n",
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(offender(&page, 4))
    );
}

#[test]
fn every_home_directory_spelling_of_a_program_is_reported() {
    for (program, contents) in [
        ("~/.local/bin", "~/.local/bin/capped cargo test\n"),
        ("$HOME", "$HOME/bin/capped just test\n"),
        ("${HOME}", "${HOME}/bin/capped just test\n"),
        ("/home/ada", "/home/ada/.local/bin/capped just test\n"),
    ] {
        let (workspace, page) = workspace("docs/build.md", contents);

        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(offender(&page, 1)),
            "{program}"
        );
    }
}

#[test]
fn a_page_version_control_does_not_track_is_accepted() {
    let (workspace, page) = workspace(
        "plans/local.md",
        "# Local\n\n```sh\n~/.local/bin/capped cargo build --locked\n```\n",
    );
    std::fs::write(workspace.root.path().join(".gitignore"), "plans/\n").unwrap();
    run_git(workspace.root.path(), &["init", "-q"]);
    run_git(workspace.root.path(), &["add", "-A"]);

    check_workspace(workspace.root.path()).unwrap();

    run_git(workspace.root.path(), &["add", "-f", "plans/local.md"]);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(offender(&page, 4))
    );
}

#[test]
fn plain_commands_and_agent_directories_are_accepted() {
    let (workspace, _) = workspace(
        "docs/build.md",
        "# Build\n\n```sh\ncargo build --locked\njust test\n```\n\nSettings live in\n`~/.maestro/agent/settings.json`, and the launcher installs into\n`~/.local/bin/`.\n",
    );

    check_workspace(workspace.root.path()).unwrap();
}
