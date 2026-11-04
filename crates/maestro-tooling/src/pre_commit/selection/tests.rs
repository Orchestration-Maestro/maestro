use super::*;

use cargo_metadata::Metadata;

/// Locked metadata of the fixture workspace these cases read.
const FIXTURE: &str = include_str!("../../../tests/fixtures/selection-metadata.json");

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

#[test]
fn staged_owners_are_selected_without_dependents() {
    for name in ["a", "b", "c", "d", "alone", "f", "external-reader"] {
        assert_packages(&[&format!("crates/{name}/src/lib.rs")], &[name]);
    }
    assert_packages(&["crates/a/src/lib.rs", "crates/g/src/lib.rs"], &["a", "g"]);
}

#[test]
fn duplicate_paths_select_each_owner_once() {
    assert_packages(
        &[
            "crates/a/src/lib.rs",
            "crates/a/src/lib.rs",
            "crates/a/tests/integration.rs",
            "crates/f/src/lib.rs",
        ],
        &["a", "f"],
    );
}

#[test]
fn native_paths_use_longest_member_root_without_filesystem_reads() {
    assert_packages(&["crates/a/nested/src/lib.rs"], &["nested"]);
    for path in [
        "crates/a/src/deleted.rs",
        "crates/a/tests/a space.rs",
        "crates/a/src/line\nbreak.rs",
        "crates/a/src/-dash.rs",
        "crates/./a/src/lib.rs",
    ] {
        assert_packages(&[path], &["a"]);
    }
    assert_packages(
        &["crates/maestro-test-conventions-old/src/lib.rs"],
        &["maestro-test-conventions-old"],
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let path = OsString::from_vec(b"crates/a/src/non-\xff.rs".to_vec());
        assert_eq!(names(&select(&parse(FIXTURE), &[path])), ["a"]);
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
        ["a"],
        "supplied checkout containing parent components"
    );
}

#[test]
fn workspace_inputs_do_not_widen_selection() {
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
        "crates/not-a-member/src/lib.rs",
        "external/a/src/lib.rs",
    ] {
        assert_packages(&[path], &[]);
        assert_packages(&[path, "crates/a/src/lib.rs"], &["a"]);
    }
    assert_packages(&[], &[]);
    assert_packages(&["crates/a/Cargo.toml"], &["a"]);
    assert_packages(
        &["crates/maestro-test-conventions/src/lib.rs"],
        &["maestro-test-conventions"],
    );
}

#[test]
fn member_order_is_preserved_in_selection() {
    assert_packages(
        &[
            "crates/g/src/lib.rs",
            "crates/f/src/lib.rs",
            "crates/a/src/lib.rs",
        ],
        &["a", "f", "g"],
    );
    assert_packages(
        &["crates/alone/src/lib.rs", "crates/a/src/lib.rs"],
        &["a", "alone"],
    );
}

#[test]
fn browser_smoke_follows_staged_consumers() {
    let metadata = parse(FIXTURE);
    for path in [
        "",
        "Cargo.toml",
        "crates/shared/src/lib.rs",
        "crates/g/src/lib.rs",
        "crates/maestro-web/src/lib.rs",
    ] {
        assert!(
            !select_paths(&metadata, &[path]).reaches_browser(),
            "{path}"
        );
    }
    for name in ["maestro-catalog", "maestro-models"] {
        let selected = select_paths(&metadata, &[&format!("crates/{name}/src/lib.rs")]);
        assert_eq!(names(&selected), [name]);
        assert!(selected.reaches_browser());
    }
}
