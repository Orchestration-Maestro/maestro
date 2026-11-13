//! Recorded cases read by the test that owns them, shared by the tests inside the library
//! and the tests that drive the interactive mode from outside.
use crate::platform_keys;
use serde::{Deserialize, de::DeserializeOwned};

/// One recorded case, read before its witness gives the input and observation their types.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: serde_json::Value,
    /// The recorded observation.
    expected: serde_json::Value,
    /// Complete macOS layout when its key names differ.
    expected_macos: Option<serde_json::Value>,
}

/// The recorded cases of `test` in `fixtures`, typed by that witness.
pub(crate) fn cases<I: DeserializeOwned, E: DeserializeOwned>(
    fixtures: &str,
    test: &str,
) -> Vec<(I, E)> {
    let all: Vec<Case> = serde_json::from_str(fixtures).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| {
            (
                serde_json::from_value(case.input).unwrap(),
                serde_json::from_value(platform_keys::for_this_platform(
                    case.expected,
                    case.expected_macos,
                ))
                .unwrap(),
            )
        })
        .collect();
    assert!(!cases.is_empty());
    cases
}
