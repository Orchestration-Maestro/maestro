//! The workflows that call the shared Rust CI.

use std::path::Path;

/// The line prefix of a call to the shared Rust CI workflow.
const SHARED_CI_CALL: &str =
    "uses: Orchestration-Maestro/maestro-rust-workflows/.github/workflows/ci.yml@";

/// Merge groups search the coverage manifest that the default concurrency key names, and the
/// full suite after landing publishes it, so no caller of the shared workflow names a key.
#[test]
fn callers_of_shared_ci_use_the_default_concurrency_key() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut callers = 0;
    for entry in std::fs::read_dir(workflows).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let calls = text
            .lines()
            .filter(|line| line.trim_start().starts_with(SHARED_CI_CALL));
        callers += calls.count();
        assert!(
            !text.contains("concurrency-key"),
            "{} names a concurrency key",
            path.display()
        );
    }
    assert!(callers >= 1, "the full-suite caller exists");
}

/// The shared workflow deletes superseded cache entries in jobs of its own, and GitHub checks
/// every nested job's permissions against the call's grant when the run starts, so each call
/// grants Actions write; the nearest Actions permission above a call is the one it grants.
#[test]
fn callers_of_shared_ci_grant_cache_pruning() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut callers = 0;
    for entry in std::fs::read_dir(workflows).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (index, line) in lines.iter().enumerate() {
            if line.starts_with(SHARED_CI_CALL) {
                callers += 1;
                let grant = lines[..index]
                    .iter()
                    .rev()
                    .find_map(|line| line.strip_prefix("actions:"));
                assert_eq!(
                    grant.map(|value| value.split('#').next().unwrap_or_default().trim()),
                    Some("write"),
                    "{} grants the shared workflow's cache pruning",
                    path.display()
                );
            }
        }
    }
    assert!(callers >= 1, "the full-suite caller exists");
}

/// The value of the first line of a workflow that sets the named key.
fn value_of<'a>(workflow: &'a str, key: &str) -> &'a str {
    workflow
        .lines()
        .filter_map(|line| line.trim().strip_prefix(key))
        .find(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("the workflow sets {key}"))
}

/// The items of a YAML flow sequence.
fn items(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .filter(|item| !item.is_empty())
        .collect()
}

/// Whether `test_inputs` of the CI settings routes `path` to `package`.
fn routed(declaration: &toml::Table, path: &str, package: &str) -> bool {
    declaration
        .get("test_inputs")
        .and_then(toml::Value::as_table)
        .and_then(|inputs| inputs.get(path))
        .and_then(toml::Value::as_array)
        .is_some_and(|packages| packages.iter().any(|name| name.as_str() == Some(package)))
}

/// Pull-request selection runs a changed crate and its dependents, and the crate-rules check
/// reads every crate's manifests and sources, so a change under `crates/` selects it; a change
/// confined to `docs/` or `fuzz/` still does not.
#[test]
fn crate_source_changes_select_the_crate_rules_check() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let declaration: toml::Table = std::fs::read_to_string(repository.join(".github/ci.toml"))
        .expect("the repository declares its CI settings")
        .parse()
        .expect("the CI settings read as TOML");
    assert!(
        routed(&declaration, "crates/", "maestro-test-conventions"),
        "a change under crates/ selects the crate-rules check"
    );
    for path in ["docs/", "fuzz/"] {
        assert!(
            !routed(&declaration, path, "maestro-test-conventions"),
            "a change under {path} does not select the crate-rules check"
        );
    }
}

/// The released commit each shared workflow call in a workflow file pins.
fn pins(workflows: &Path, name: &str) -> Vec<String> {
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
}

/// The listener condition passes completed pull-request and merge-group runs, and main pushes
/// only on the default branch.
fn listener_passes_every_run_event(listener: &str) {
    let condition = listener.split_whitespace().collect::<Vec<_>>().join(" ");
    for clause in [
        "github.event.workflow_run.event == 'pull_request'",
        "github.event.workflow_run.event == 'merge_group'",
        "(github.event.workflow_run.event == 'push' && github.event.workflow_run.head_branch == github.event.repository.default_branch)",
    ] {
        assert!(
            condition.contains(clause),
            "the listener passes the completed runs of {clause}"
        );
    }
}

/// Every watched workflow this repository declares calls the shared CI, so the guards read the job
/// names behind the caller's prefix instead of reporting them missing.
fn watched_workflows_call_shared_ci(workflows: &Path, watched: &[String]) {
    for (file, name) in declared(workflows) {
        if !watched.contains(&name) {
            continue;
        }
        let text = std::fs::read_to_string(&file).expect("the watched workflow reads");
        assert!(
            text.lines()
                .any(|line| line.trim_start().starts_with(SHARED_CI_CALL)),
            "the listener watches {name}, which {file} declares without calling the shared CI"
        );
    }
}

/// The workflow name each file in the workflow directory declares.
fn declared(workflows: &Path) -> Vec<(String, String)> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(workflows).expect("the workflow directory reads") {
        let path = entry.expect("the workflow entry reads").path();
        let text = std::fs::read_to_string(&path).expect("the workflow file reads");
        if let Some(name) = text.lines().find_map(|line| line.strip_prefix("name: ")) {
            names.push((path.display().to_string(), name.trim().to_owned()));
        }
    }
    names
}

/// A manual browser proof runs only the Chromium job, on the named ref, at the release the full
/// suite runs, so a red and green pair proves the code the merge queue will test.
#[test]
fn the_browser_proof_runs_the_suites_release_on_the_named_ref() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    assert_eq!(
        pins(&workflows, "browser-proof.yml"),
        pins(&workflows, "full-suite.yml"),
        "the proof pins the suite's release"
    );
    let proof = std::fs::read_to_string(workflows.join("browser-proof.yml"))
        .expect("the browser proof caller exists");
    assert_eq!(value_of(&proof, "browser-only:").trim(), "true");
    assert_eq!(
        value_of(&proof, "checkout-ref:").trim(),
        "${{ inputs.ref }}"
    );
}

/// Every caller pins one released commit, and the guards listener passes the pinned guards every
/// run they judge: merge groups, pull requests and the full suite after landing.
#[test]
fn the_guards_listener_judges_completed_runs_at_the_callers_pin() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workflows = repository.join(".github/workflows");
    let listener = std::fs::read_to_string(workflows.join("ci-guards.yml"))
        .expect("the speed guards listener exists");
    let suite = pins(&workflows, "full-suite.yml");
    let landing = pins(&workflows, "post-landing.yml");
    for revision in pins(&workflows, "ci-guards.yml") {
        assert_eq!(
            suite,
            [revision.as_str()],
            "the suite caller pins this release"
        );
        assert_eq!(
            landing,
            [revision.as_str()],
            "the landing caller pins it too"
        );
    }
    assert_eq!(
        items(value_of(&listener, "types:")),
        ["completed"],
        "the listener reads a finished run"
    );
    let watched = items(value_of(&listener, "workflows:"));
    assert_eq!(
        watched,
        ["Rust CI", "Full suite after landing"],
        "the listener watches the merge groups, pull requests and the full suite"
    );
    // The pinned guards read the job names a reusable caller prefixes with its own job name, so a
    // watched workflow this repository declares must be a caller of the shared CI.
    watched_workflows_call_shared_ci(&workflows, &watched);
    listener_passes_every_run_event(&listener);
    assert!(
        value_of(&listener, "run-id:").contains("github.event.workflow_run.id"),
        "the judged run is the completed one"
    );
    assert!(
        listener.lines().any(|line| line.trim() == "pull_request:"),
        "pull requests run the test-time exception check"
    );
    let declaration: toml::Table = std::fs::read_to_string(repository.join(".github/ci.toml"))
        .expect("the repository declares its CI settings")
        .parse()
        .expect("the CI settings read as TOML");
    let privileged = declaration
        .get("privileged_trigger_workflows")
        .and_then(toml::Value::as_array)
        .expect("the CI settings declare the privileged trigger workflows");
    assert!(
        privileged
            .iter()
            .any(|file| file.as_str() == Some(".github/workflows/ci-guards.yml")),
        "the trigger audit exempts this listener"
    );
}
