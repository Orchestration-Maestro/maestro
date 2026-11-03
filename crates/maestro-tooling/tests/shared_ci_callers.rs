//! The workflows that call the shared Rust CI.

use std::path::Path;

/// Merge groups search the coverage manifest that the default concurrency key names, and the
/// full suite after landing publishes it, so no caller of the shared workflow names a key.
#[test]
fn callers_of_shared_ci_use_the_default_concurrency_key() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut callers = 0;
    for entry in std::fs::read_dir(workflows).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let calls = text.lines().filter(|line| {
            line.trim_start().starts_with(
                "uses: Orchestration-Maestro/maestro-rust-workflows/.github/workflows/ci.yml@",
            )
        });
        callers += calls.count();
        assert!(
            !text.contains("concurrency-key"),
            "{} names a concurrency key",
            path.display()
        );
    }
    assert!(callers >= 1, "the full-suite caller exists");
}

/// Every caller pins one released commit, and the guards listener judges both the merge groups and
/// the default-branch pushes whose compile snapshots the other runs restore.
#[test]
fn the_guards_listener_judges_completed_runs_at_the_callers_pin() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workflows = repository.join(".github/workflows");
    let pins = |name: &str| -> Vec<String> {
        let text = std::fs::read_to_string(workflows.join(name)).expect("the caller reads");
        let found: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_prefix("uses: "))
            .map(|call| {
                call.rsplit_once('@')
                    .expect("the call is pinned")
                    .1
                    .split_whitespace()
                    .next()
                    .expect("the pin precedes the comment")
                    .to_owned()
            })
            .collect();
        assert!(!found.is_empty(), "{name} calls the shared workflows");
        found
    };
    let listener = std::fs::read_to_string(workflows.join("ci-guards.yml"))
        .expect("the speed guards listener exists");
    for revision in pins("ci-guards.yml") {
        assert_eq!(pins("full-suite.yml"), [revision.as_str()]);
        assert_eq!(pins("post-landing.yml"), [revision.as_str()]);
    }
    assert!(
        listener.contains("    types: [completed]\n"),
        "the listener reads a finished run"
    );
    assert!(
        listener.contains("workflows: [Rust CI, Full suite after landing]\n"),
        "merge groups and default-branch pushes are judged"
    );
    assert!(
        listener.contains("      run-id: ${{ format('{0}', github.event.workflow_run.id) }}\n"),
        "the judged run is the completed one"
    );
    assert!(
        listener.contains("  pull_request:\n"),
        "pull requests run the test-time exception check"
    );
    let declaration = std::fs::read_to_string(repository.join(".github/ci.toml")).unwrap();
    let privileged = declaration.lines().next().unwrap();
    assert!(
        privileged.contains("\".github/workflows/ci-guards.yml\""),
        "the trigger audit ignores this listener"
    );
}
