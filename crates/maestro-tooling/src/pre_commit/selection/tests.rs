use super::{Scope, select};

use std::ffi::OsString;

use cargo_metadata::Metadata;

/// Locked metadata of the fixture workspace these cases read.
const FIXTURE: &str = include_str!("../../../tests/fixtures/selection-metadata.json");

/// Fixture members in metadata order.
const MEMBERS: &[&str] = &[
    "a",
    "f",
    "b",
    "c",
    "d",
    "e",
    "g",
    "alone",
    "nested",
    "maestro-test-conventions-old",
    "external-reader",
    "shared",
    "maestro-models",
    "maestro-web",
];

/// Members reached by a change to `a`: its reverse dependents in metadata order.
const REACHED_FROM_A: &[&str] = &["a", "f", "b", "c", "d", "e", "g"];

/// Parse a metadata document.
fn parse(document: &str) -> Metadata {
    serde_json::from_str(document).unwrap()
}

/// Select the fixture members for these staged paths.
fn select_paths(metadata: &Metadata, paths: &[&str]) -> Scope {
    let native = paths.iter().map(OsString::from).collect::<Vec<_>>();
    select(metadata, &native)
}

/// Selected member names.
fn names(scope: &Scope) -> Vec<&str> {
    scope.names().iter().map(String::as_str).collect()
}

/// Assert the members a staged change selects.
fn assert_packages(paths: &[&str], expected: &[&str]) {
    assert_eq!(names(&select_paths(&parse(FIXTURE), paths)), expected);
}

/// Assert the whole workspace a staged change selects.
fn assert_workspace(paths: &[&str]) {
    assert_eq!(
        select_paths(&parse(FIXTURE), paths),
        Scope::Workspace(MEMBERS.iter().map(|name| (*name).to_owned()).collect())
    );
}

#[test]
fn changed_owner_and_transitive_callers_are_selected() {
    assert_packages(&["crates/a/src/lib.rs"], REACHED_FROM_A);
    assert_packages(&["crates/b/src/lib.rs"], &["b", "d"]);
    assert_packages(&["crates/c/src/lib.rs"], &["c", "d"]);
    assert_packages(&["crates/d/src/lib.rs"], &["d"]);
    assert_packages(&["crates/alone/src/lib.rs"], &["alone"]);
    assert_packages(
        &["crates/a/src/lib.rs", "crates/g/src/lib.rs"],
        REACHED_FROM_A,
    );
    assert_packages(
        &["crates/a/src/lib.rs", "crates/alone/src/lib.rs"],
        &["a", "f", "b", "c", "d", "e", "g", "alone"],
    );
}

#[test]
fn declared_edge_kinds_keep_aliases_and_inactive_callers() {
    assert_packages(&["crates/f/src/lib.rs"], REACHED_FROM_A);
    assert_packages(&["crates/e/src/lib.rs"], &["e"]);
    assert_packages(&["crates/./a/src/lib.rs"], REACHED_FROM_A);
    let reached = select_paths(&parse(FIXTURE), &["crates/a/src/lib.rs"]);
    assert!(!names(&reached).contains(&"external-reader"));
    assert_packages(&["crates/external-reader/src/lib.rs"], &["external-reader"]);
}

#[test]
fn duplicate_edges_and_cycles_schedule_each_member_once() {
    for paths in [
        vec!["crates/a/src/lib.rs"],
        vec!["crates/f/src/lib.rs"],
        vec!["crates/a/src/lib.rs", "crates/a/src/lib.rs"],
        vec!["crates/a/src/lib.rs", "crates/f/src/lib.rs"],
    ] {
        let selected = select_paths(&parse(FIXTURE), &paths);
        assert_eq!(names(&selected).len(), 7, "{paths:?}");
        assert_packages(&paths, REACHED_FROM_A);
    }
}

#[test]
fn native_paths_use_longest_member_root_without_filesystem_reads() {
    assert_packages(&["crates/a/nested/src/lib.rs"], &["nested"]);
    assert_packages(&["crates/a/src/deleted.rs"], REACHED_FROM_A);
    assert_packages(&["crates/a/tests/a space.rs"], REACHED_FROM_A);
    assert_packages(&["crates/a/src/line\nbreak.rs"], REACHED_FROM_A);
    assert_packages(&["crates/a/src/-dash.rs"], REACHED_FROM_A);
    assert_packages(
        &["crates/maestro-test-conventions-old/src/lib.rs"],
        &["maestro-test-conventions-old"],
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let path = OsString::from_vec(b"crates/a/src/non-\xff.rs".to_vec());
        let reached = select(&parse(FIXTURE), &[path]);
        assert_eq!(names(&reached), REACHED_FROM_A, "native bytes");
    }
    let nested_checkout = FIXTURE.replace(
        "\"workspace_root\": \"/maestro/fixture\"",
        "\"workspace_root\": \"/maestro/./fixture/../fixture\"",
    );
    assert_eq!(
        names(&select_paths(
            &parse(&nested_checkout),
            &["crates/a/src/lib.rs"]
        )),
        REACHED_FROM_A,
        "supplied checkout containing parent components"
    );
}

#[test]
fn workspace_inputs_and_unknown_paths_select_every_member() {
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/config.toml",
        "rust-toolchain.toml",
        "justfile",
        "clippy.toml",
        "mise.toml",
        "mise.lock",
        ".pre-commit-config.yaml",
        ".github/ci.toml",
        ".github/workflows/check.yml",
        "workspace-crates.json",
        "docs/development.md",
        "docs/specs/maestro-port.md",
        "crates/maestro-test-conventions/src/lib.rs",
        "crates/maestro-test-conventions/tests/integration.rs",
        "crates/maestro-test-conventions/tests/fixtures/row.rs",
        "crates/a/Cargo.toml",
        "crates/not-a-member/src/lib.rs",
        "external/a/src/lib.rs",
    ] {
        assert_workspace(&[path]);
        assert_workspace(&[path, "crates/a/src/lib.rs"]);
    }
    assert_workspace(&[]);
    assert_packages(
        &["crates/maestro-test-conventions-old/src/lib.rs"],
        &["maestro-test-conventions-old"],
    );
}

#[test]
fn member_order_is_preserved_in_selection() {
    let mut sorted = REACHED_FROM_A.to_vec();
    sorted.sort_unstable();
    assert_ne!(REACHED_FROM_A, sorted, "fixture order is alphabetical");
    assert_packages(&["crates/a/src/lib.rs"], REACHED_FROM_A);
    assert_packages(
        &["crates/alone/src/lib.rs", "crates/a/src/lib.rs"],
        &["a", "f", "b", "c", "d", "e", "g", "alone"],
    );
}

#[test]
fn browser_smoke_follows_selected_consumers() {
    let metadata = parse(FIXTURE);
    assert!(select_paths(&metadata, &[]).reaches_browser());
    let shared = select_paths(&metadata, &["crates/shared/src/lib.rs"]);
    assert_eq!(names(&shared), ["shared", "maestro-models", "maestro-web"]);
    assert!(shared.reaches_browser());
    let models = select_paths(&metadata, &["crates/maestro-models/src/lib.rs"]);
    assert_eq!(names(&models), ["maestro-models", "maestro-web"]);
    assert!(models.reaches_browser());
    assert!(!select_paths(&metadata, &["crates/alone/src/lib.rs"]).reaches_browser());
    assert!(!select_paths(&metadata, &["crates/g/src/lib.rs"]).reaches_browser());
}
