#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;

/// Write runner configuration beside the fixture workspace manifest.
fn configure(workspace: &Workspace, config: &str) {
    let directory = workspace.root.path().join(".config");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("nextest.toml"), config).unwrap();
}

#[test]
fn nextest_rejects_raised_or_unbounded_default_budget() {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    for timeout in [
        "{ period = '31s', terminate-after = 1 }",
        "{ period = '30s', terminate-after = 2 }",
        "{ period = '30s' }",
        "'30s'",
        "{ period = '30s', terminate-after = 0 }",
    ] {
        configure(
            &workspace,
            &format!("[profile.default]\nslow-timeout = {timeout}\n"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("nextest.toml"), "{error}");
        assert!(error.contains("30-second budget"), "{timeout}: {error}");
    }
}

#[test]
fn nextest_rejects_undocumented_slow_exceptions() {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    for comment in [
        "",
        "# Measured: 43.549s; cause: repeated metadata subprocesses.",
        "# #3589",
        "# Measured: 43.549s; cause: repeated metadata subprocesses; #0",
        "# Measured: 43.549s; cause: ; #3589",
    ] {
        configure(
            &workspace,
            &format!(
                "[profile.default]\nslow-timeout = {{ period = '30s', terminate-after = 1 }}\n\n[[profile.default.overrides]]\nfilter = 'test(=slow)'\n{comment}\nslow-timeout = {{ period = '44s', terminate-after = 1 }}\n"
            ),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("measured cause and issue (#NNNN)"),
            "{error}"
        );
        assert!(error.contains("nextest.toml"), "{error}");
    }
}

#[test]
fn nextest_accepts_bounded_defaults_and_documented_exceptions() {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    configure(
        &workspace,
        "[profile.default]\nslow-timeout = { period = '15s', terminate-after = 2 }\n\n[[profile.default.overrides]]\nfilter = 'test(=slow)'\n# Measured: 43.549s; cause: repeated metadata subprocesses; #3589\nslow-timeout = { period = '44s', terminate-after = 1 }\n\n[[profile.default.overrides]]\nfilter = 'test(=fast)'\nslow-timeout = { period = '5s', terminate-after = 1 }\n",
    );
    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn nextest_exception_comment_does_not_leak_to_another_override() {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    configure(
        &workspace,
        "[profile.default]\nslow-timeout = { period = '30s', terminate-after = 1 }\n\n[[profile.default.overrides]]\nfilter = 'test(=first)'\n# Measured: 43.549s; cause: repeated metadata subprocesses; #3589\nslow-timeout = { period = '44s', terminate-after = 1 }\n\n[[profile.default.overrides]]\nfilter = 'test(=second)'\nslow-timeout = { period = '44s', terminate-after = 1 }\n",
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("measured cause and issue (#NNNN)"),
        "{error}"
    );
}

#[test]
fn nextest_rejects_unbounded_exceptions_and_raised_other_profiles() {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    for section in [
        "[profile.ci]\nslow-timeout = { period = '31s', terminate-after = 1 }",
        "[[profile.default.overrides]]\nfilter = 'test(=slow)'\nslow-timeout = { period = '31s' }",
    ] {
        configure(
            &workspace,
            &format!(
                "[profile.default]\nslow-timeout = {{ period = '30s', terminate-after = 1 }}\n\n# Measured: 30.5s; cause: metadata subprocesses; #3589\n{section}\n"
            ),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("30-second budget"), "{error}");
    }
}
