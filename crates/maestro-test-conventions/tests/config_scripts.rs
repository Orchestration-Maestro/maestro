#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;

/// A fixture workspace with one member and a runner configuration directory.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    std::fs::create_dir_all(workspace.root.path().join(".config")).unwrap();
    workspace
}

#[test]
fn runner_configuration_rejects_a_shell_script() {
    let workspace = workspace();
    let script = workspace.root.path().join(".config/prepare.sh");
    std::fs::write(&script, "#!/bin/sh\n").unwrap();

    let error = check_workspace(workspace.root.path()).unwrap_err();

    assert!(error.contains(".config/prepare.sh"), "{error}");
    assert!(error.contains("development tool"), "{error}");
}

#[test]
fn runner_configuration_accepts_files_that_are_not_shell_scripts() {
    let workspace = workspace();
    let directory = workspace.root.path().join(".config");
    std::fs::write(
        directory.join("nextest.toml"),
        "[profile.default]\nslow-timeout = { period = '30s', terminate-after = 4 }\n",
    )
    .unwrap();
    std::fs::write(directory.join("hakari.toml"), "").unwrap();
    std::fs::write(directory.join("shell.txt"), "").unwrap();

    check_workspace(workspace.root.path()).unwrap();
}
