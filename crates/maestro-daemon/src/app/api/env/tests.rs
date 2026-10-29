//! Launch environment validation before any root pane starts.
use super::normalize_launch_env;
use std::collections::HashMap;

/// The pairs `env` normalizes to, or its failure as code and message.
fn normalized(env: &[(&str, &str)]) -> Result<Vec<(String, String)>, (String, String)> {
    let env = env
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect::<HashMap<_, _>>();
    normalize_launch_env(env).map_err(|error| (error.code, error.message))
}

/// The pairs of `env` as owned strings, in the given order.
fn pairs(env: &[(&str, &str)]) -> Vec<(String, String)> {
    env.iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn launch_environment_is_sorted_by_key() {
    assert_eq!(normalized(&[]), Ok(Vec::new()));
    assert_eq!(
        normalized(&[("ZED", "last"), ("ALPHA", "first"), ("É", ""), ("B", "é")]),
        Ok(pairs(&[
            ("ALPHA", "first"),
            ("B", "é"),
            ("ZED", "last"),
            ("É", "")
        ]))
    );
    // One key with several faults reports the first check it fails.
    let invalid = |message: &str| Err(("invalid_env".to_owned(), message.to_owned()));
    assert_eq!(
        normalized(&[("A=\0", "\0")]),
        invalid("env key A=\0 must not contain '='")
    );
    assert_eq!(
        normalized(&[("A\0", "\0")]),
        invalid("env key must not contain NUL bytes")
    );
}

#[test]
fn launch_environment_rejects_all_native_invalid_operands() {
    let invalid = |message: &str| Err(("invalid_env".to_owned(), message.to_owned()));
    for (env, expected) in [
        (("", "value"), invalid("env key must not be empty")),
        (
            ("BAD=KEY", "value"),
            invalid("env key BAD=KEY must not contain '='"),
        ),
        (
            ("BAD\0KEY", "value"),
            invalid("env key must not contain NUL bytes"),
        ),
        (
            ("KEY", "bad\0value"),
            invalid("env value for KEY must not contain NUL bytes"),
        ),
    ] {
        assert_eq!(normalized(&[env]), expected, "{env:?}");
    }
    assert_eq!(normalized(&[("KEY", "")]), Ok(pairs(&[("KEY", "")])));
}
