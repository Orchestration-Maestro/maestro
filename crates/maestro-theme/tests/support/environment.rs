//! Child environments for theme integration tests.
use std::collections::BTreeMap;

/// A truecolor terminal, so theme loads without an explicit mode emit RGB.
pub const TRUECOLOR: &[(&str, &str)] = &[("COLORTERM", "truecolor")];

/// Run cases through [`maestro_test_process::run_per_environment`], which keeps the
/// temporary directory unless a case supplies one. Checked-child selection and
/// instrumentation variables are also supplied. Return this child's cases, or `None`
/// in the calling process once every child passed.
///
/// # Panics
/// Panics if a child fails or does not start exactly `test`.
pub fn with_observations<C>(
    test: &str,
    cases: impl IntoIterator<Item = C>,
    environment: impl Fn(&C) -> &[(&'static str, &'static str)],
) -> Option<Vec<C>> {
    maestro_test_process::run_per_environment(test, cases, |case| {
        environment(case)
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<String, String>>()
    })
    .unwrap()
}

/// Whether this process runs `test`'s body under [`with_observations`].
pub fn in_child(test: &str, env: &'static [(&'static str, &'static str)]) -> bool {
    with_observations(test, [env], |env| env).is_some()
}
