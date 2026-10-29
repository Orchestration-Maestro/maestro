#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;

/// The default profile that flags at thirty seconds and kills at the hang limit.
const DEFAULT: &str = "[profile.default]\nslow-timeout = { period = '30s', terminate-after = 4 }\n";

/// Write runner configuration beside the fixture workspace manifest.
fn configure(workspace: &Workspace, config: &str) {
    let directory = workspace.root.path().join(".config");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("nextest.toml"), config).unwrap();
}

/// A fixture workspace with one member, ready for runner configuration.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    workspace
}

/// One override selecting `slow` with `timeout`, preceded by `comment`.
fn exception(comment: &str, timeout: &str) -> String {
    format!(
        "{DEFAULT}\n[[profile.default.overrides]]\nfilter = 'test(=slow)'\n{comment}\nslow-timeout = {timeout}\n"
    )
}

#[test]
fn nextest_rejects_a_default_that_is_not_the_flag_and_hang_limit() {
    let workspace = workspace();
    for timeout in [
        "{ period = '30s', terminate-after = 1 }",
        "{ period = '30s', terminate-after = 3 }",
        "{ period = '30s', terminate-after = 5 }",
        "{ period = '60s', terminate-after = 2 }",
        "{ period = '15s', terminate-after = 8 }",
        "{ period = '31s', terminate-after = 4 }",
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
        assert!(
            error.contains("30-second slow flag and a 120-second hang limit"),
            "{timeout}: {error}"
        );
    }
    configure(&workspace, "[profile.default]\n");
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("30-second slow flag and a 120-second hang limit"),
        "{error}"
    );
}

#[test]
fn nextest_rejects_undocumented_slow_exceptions() {
    let workspace = workspace();
    for comment in [
        "",
        "# Measured: 143.549s; cause: repeated metadata subprocesses.",
        "# #3589",
        "# Measured: 143.549s; cause: repeated metadata subprocesses; #0",
        "# Measured: 143.549s; cause: ; #3589",
        "# Measured: 90.0s; cause: repeated metadata subprocesses; #3589",
    ] {
        configure(
            &workspace,
            &exception(comment, "{ period = '30s', terminate-after = 5 }"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("measured cause and issue (#NNNN)"),
            "{comment}: {error}"
        );
        assert!(error.contains("nextest.toml"), "{error}");
    }
}

#[test]
fn nextest_rejects_exceptions_the_hang_limit_covers() {
    let workspace = workspace();
    for timeout in [
        "{ period = '30s', terminate-after = 4 }",
        "{ period = '30s', terminate-after = 1 }",
    ] {
        configure(
            &workspace,
            &exception(
                "# Measured: 143.549s; cause: repeated metadata subprocesses; #3589",
                timeout,
            ),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("covered by the 120-second hang limit"),
            "{timeout}: {error}"
        );
    }
}

#[test]
fn nextest_rejects_exceptions_that_drop_the_slow_flag() {
    let workspace = workspace();
    for timeout in [
        "{ period = '200s', terminate-after = 1 }",
        "{ period = '60s', terminate-after = 3 }",
        "{ period = '5s', terminate-after = 40 }",
    ] {
        configure(
            &workspace,
            &exception(
                "# Measured: 143.549s; cause: repeated metadata subprocesses; #3589",
                timeout,
            ),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("exception must keep the 30-second slow flag"),
            "{timeout}: {error}"
        );
    }
}

#[test]
fn nextest_accepts_the_flag_and_documented_exceptions_over_the_hang_limit() {
    let workspace = workspace();
    configure(
        &workspace,
        &format!(
            "{DEFAULT}\n[profile.ci]\nslow-timeout = {{ period = '30s', terminate-after = 2 }}\n\n[[profile.default.overrides]]\nfilter = 'test(=slow)'\n# Measured: 143.549s; cause: repeated metadata subprocesses; #3589\nslow-timeout = {{ period = '30s', terminate-after = 5 }}\n\n[[profile.default.overrides]]\nfilter = 'test(=grouped)'\ntest-group = 'serial'\n"
        ),
    );
    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn nextest_exception_comment_does_not_leak_to_another_override() {
    let workspace = workspace();
    configure(
        &workspace,
        &format!(
            "{DEFAULT}\n[[profile.default.overrides]]\nfilter = 'test(=first)'\n# Measured: 143.549s; cause: repeated metadata subprocesses; #3589\nslow-timeout = {{ period = '30s', terminate-after = 5 }}\n\n[[profile.default.overrides]]\nfilter = 'test(=second)'\nslow-timeout = {{ period = '30s', terminate-after = 5 }}\n"
        ),
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("measured cause and issue (#NNNN)"),
        "{error}"
    );
}

#[test]
fn nextest_rejects_unbounded_exceptions_and_raised_other_profiles() {
    let workspace = workspace();
    for (section, expected) in [
        (
            "[profile.ci]\nslow-timeout = { period = '30s', terminate-after = 5 }",
            "profile ci must keep the 30-second slow flag and the 120-second hang limit",
        ),
        (
            "[profile.ci]\nslow-timeout = { period = '31s', terminate-after = 1 }",
            "profile ci must keep the 30-second slow flag and the 120-second hang limit",
        ),
        (
            "[[profile.default.overrides]]\nfilter = 'test(=slow)'\nslow-timeout = { period = '30s' }",
            "positive whole seconds",
        ),
    ] {
        configure(
            &workspace,
            &format!(
                "{DEFAULT}\n# Measured: 143.549s; cause: metadata subprocesses; #3589\n{section}\n"
            ),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains(expected), "{section}: {error}");
    }
}
