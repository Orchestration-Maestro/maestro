//! The agent directory a cleared child names to share its parent's compiled code.
#![cfg(test)]
#![cfg(unix)]

use std::collections::BTreeMap;

use maestro_test_harness::{AGENT_DIR, run_agent_dir, run_per_environment};

/// One case: the variable it runs with and the directory it reports.
type Case = (Option<&'static str>, Option<&'static str>);

#[test]
fn run_agent_directory_reports_only_an_absolute_exported_path() {
    let cases: [Case; 3] = [
        (Some("/agent"), Some("/agent")),
        (Some("relative"), None),
        (None, None),
    ];
    let selected = run_per_environment(
        "run_agent_directory_reports_only_an_absolute_exported_path",
        cases,
        |(agent, _)| BTreeMap::from([(AGENT_DIR, *agent)]),
    )
    .unwrap();
    for (_, expected) in selected.into_iter().flatten() {
        assert_eq!(run_agent_dir().as_deref(), expected);
    }
}
