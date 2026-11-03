//! The user cache location a cleared child names to share its parent's compiled code.
#![cfg(test)]
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use maestro_test_process::{run_per_environment, user_cache_home};

/// One case: the variables it runs with and the location it reports.
type Case = (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
);

#[test]
fn user_cache_home_follows_the_cache_variable_then_home() {
    let cases: [Case; 4] = [
        (Some("/xdg"), Some("/home/al"), Some("/xdg")),
        (None, Some("/home/al"), Some("/home/al/.cache")),
        (Some("relative"), Some("/home/al"), Some("/home/al/.cache")),
        (None, None, None),
    ];
    let selected = run_per_environment(
        "user_cache_home_follows_the_cache_variable_then_home",
        cases,
        |(xdg, home, _)| {
            [("XDG_CACHE_HOME", xdg), ("HOME", home)]
                .into_iter()
                .filter_map(|(name, value)| Some((name, (*value)?)))
                .collect::<BTreeMap<_, _>>()
        },
    )
    .unwrap();
    for (_, _, expected) in selected.into_iter().flatten() {
        assert_eq!(user_cache_home(), expected.map(PathBuf::from));
    }
}
