//! Shared fixtures for controlled terminal scenarios.

pub use maestro_test_terminal::{manual_runtime, recording_terminal};

/// Runs `body` in one cleared child whose environment is exactly `environment`.
///
/// The scenario re-runs as `test`, so a witness that reads process variables states them
/// here instead of inheriting the runner's.
pub fn in_child(test: &str, environment: &[(&str, &str)], body: impl FnOnce()) {
    let cases = match maestro_test_process::run_per_environment(test, [()], |()| {
        environment
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<std::collections::BTreeMap<_, _>>()
    }) {
        Ok(Some(cases)) => cases,
        Ok(None) => return,
        Err(error) => panic!("{error}"),
    };
    assert_eq!(cases.len(), 1);
    body();
}

pub mod checks;
pub mod components;
pub mod markdown_cases;
pub mod scene;
pub mod virtual_terminal;
pub mod writer;
