//! Native Git test commands use the isolated command owner.

use crate::support::{Workspace, source};
use maestro_test_conventions::check_workspace;

/// Check a test or production source containing the supplied command expression.
fn check(file: &str, command: &str) -> Result<(), String> {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-text"]);
    source(
        &workspace,
        "maestro-text",
        file,
        &format!("//! Commands.\n/// Prepare a command.\nfn command() {{ let _ = {command}; }}\n"),
    );
    if file.starts_with("tests/") {
        workspace.integration("maestro-text");
    }
    check_workspace(workspace.root.path())
}

/// Test constructors are rejected while owner calls and production constructors remain valid.
#[test]
fn tests_reject_direct_git_commands() {
    for command in [
        "std::process::Command::new(\"git\")",
        "Command::new(\"git.exe\")",
        "NativeCommand::new(\"git\")",
    ] {
        let error = check("tests/commands.rs", command).unwrap_err();
        assert!(
            error.contains("maestro_test_process::git_command::command"),
            "{error}"
        );
    }
    assert_eq!(
        check(
            "tests/commands.rs",
            "maestro_test_process::git_command::command()"
        ),
        Ok(())
    );
    assert_eq!(
        check("src/commands.rs", "std::process::Command::new(\"git\")"),
        Ok(())
    );
}

/// Macro arguments carry the same constructor boundary as ordinary test expressions.
#[test]
fn tests_reject_git_commands_inside_macros() {
    let error = check(
        "tests/commands.rs",
        "assert!(Command::new(\"git\").status().is_ok())",
    )
    .unwrap_err();
    assert!(
        error.contains("maestro_test_process::git_command::command"),
        "{error}"
    );
}
