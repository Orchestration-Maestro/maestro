//! Unit coverage for the environment filter of the preparation build.

use std::ffi::OsString;
use std::path::Path;

use super::{inherited_components, listed};

/// Every name component discovery accepts is removed, including the bare and doubled
/// underscore forms, and no other name is.
#[test]
fn inherited_components_follow_the_discovery_rule() {
    let names = [
        "MAESTRO_AUTHOR_COMPONENT",
        "MAESTRO_COMPONENT",
        "MAESTRO__COMPONENT",
        "MAESTRO_HOME",
        "MAESTRO_lower_COMPONENT",
        "PATH",
    ]
    .map(OsString::from);
    assert_eq!(
        inherited_components(names.into_iter()),
        [
            "MAESTRO_AUTHOR_COMPONENT",
            "MAESTRO_COMPONENT",
            "MAESTRO__COMPONENT"
        ]
        .map(OsString::from)
    );
}

/// Whether a `test_prepare_inputs` pattern covers a file: the pattern names the file, or it is a
/// directory followed by `/**`, which covers everything below the directory at any depth.
fn covers(pattern: &str, name: &str) -> bool {
    pattern
        .strip_suffix("/**")
        .map_or(pattern == name, |directory| {
            name.strip_prefix(directory)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

/// Shared CI keys the cached bundle on the files these patterns match, so a file the preparation
/// reads that no pattern covers could change while a stale bundle is restored, and a pattern that
/// matches nothing fails the job.
#[test]
fn ci_cache_key_covers_every_file_the_preparation_reads() {
    let ci: toml::Table = include_str!("../../../../.github/ci.toml").parse().unwrap();
    let patterns: Vec<&str> = ci["test_prepare_inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pattern| pattern.as_str().unwrap())
        .collect();
    for pattern in &patterns {
        let literal = pattern.strip_suffix("/**").unwrap_or(pattern);
        assert!(
            !literal.contains(['*', '?', '[', '{']),
            "unsupported {pattern}"
        );
    }
    let read = listed(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();

    let uncovered: Vec<&String> = read
        .iter()
        .filter(|name| !patterns.iter().any(|pattern| covers(pattern, name)))
        .collect();

    assert!(uncovered.is_empty(), "no pattern covers {uncovered:?}");
    for pattern in &patterns {
        assert!(
            read.iter().any(|name| covers(pattern, name)),
            "{pattern} matches no file the preparation reads"
        );
    }
}
