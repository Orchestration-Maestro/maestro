//! Child environments for theme integration tests.
use std::collections::BTreeMap;

/// A truecolor terminal, so theme loads without an explicit mode emit RGB.
pub const TRUECOLOR: &[(&str, &str)] = &[("COLORTERM", "truecolor")];

/// Run cases through [`maestro_test_process::run_per_environment`], retaining temporary
/// directory variables unless explicitly supplied. Checked-child selection and
/// instrumentation variables are also supplied. Return this child's cases, or `None`
/// in the calling process once every child passed.
///
/// # Panics
/// Panics if a child fails or does not start exactly `test`.
pub fn per_environment<C>(
    test: &str,
    cases: impl IntoIterator<Item = C>,
    environment: impl Fn(&C) -> &[(&'static str, &'static str)],
) -> Option<Vec<C>> {
    maestro_test_process::run_per_environment(test, cases, |case| {
        let mut env: BTreeMap<String, String> = environment(case)
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        // The temporary root stays, so a child can still create scratch directories.
        for name in ["TMPDIR", "TMP", "TEMP"] {
            if let Ok(value) = std::env::var(name) {
                env.entry(name.to_owned()).or_insert(value);
            }
        }
        env
    })
    .unwrap()
}

/// Whether this process runs `test`'s body under [`per_environment`].
pub fn in_child(test: &str, env: &'static [(&'static str, &'static str)]) -> bool {
    per_environment(test, [env], |env| env).is_some()
}
