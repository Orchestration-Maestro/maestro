//! The words of a load failure.
use crate::failure::{Failure, no_manifest_root};

#[test]
fn a_failure_names_the_file_the_source_the_problem_and_the_fix() {
    let failure = Failure {
        file: "/home/me/.maestro/agent/settings.json".into(),
        source: "npm:@acme/manifests".into(),
        problem: "it is not a Git source pinned to a tag or commit".into(),
        fix: "set `manifest` to a Git source pinned to a tag or commit".into(),
    };
    assert_eq!(
        failure.to_string(),
        "/home/me/.maestro/agent/settings.json: npm:@acme/manifests: it is not a Git source \
         pinned to a tag or commit. Fix: set `manifest` to a Git source pinned to a tag or commit"
    );
}

#[test]
fn a_release_without_a_manifest_root_is_named_by_its_ref() {
    assert_eq!(
        no_manifest_root("git:github.com/acme/manifests@v1.2.0"),
        "release v1.2.0 has no manifests/ folder"
    );
    assert_eq!(
        no_manifest_root("git@github.com:acme/manifests@abc123"),
        "release abc123 has no manifests/ folder"
    );
}
