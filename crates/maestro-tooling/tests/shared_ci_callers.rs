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
            if line.starts_with(
                "uses: Orchestration-Maestro/maestro-rust-workflows/.github/workflows/ci.yml@",
            ) {
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

/// Every caller pins one released commit, and the guards listener judges the runs whose job names
/// the pinned guards compare exactly.
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
    assert!(!watched.is_empty(), "the listener awaits a workflow's runs");
    // The pinned guards match job names exactly, and a reusable caller names every job after the
    // calling one, so a run of a workflow this repository declares reads as missing checks.
    for (file, name) in declared(&workflows) {
        assert!(
            !watched.contains(&name),
            "the listener watches {name}, which {file} declares"
        );
    }
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
