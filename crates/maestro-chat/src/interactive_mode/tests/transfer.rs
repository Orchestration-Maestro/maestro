//! The export command's path argument and the installed submission that runs it.
use super::super::transfer::get_path_command_argument;
use serde::{Deserialize, de::DeserializeOwned};

/// Every recorded argument case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_transfer.json");

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
}

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    let all: Vec<Case> = serde_json::from_str(FIXTURES).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| {
            (
                serde_json::from_value(case.input).unwrap(),
                serde_json::from_value(case.expected).unwrap(),
            )
        })
        .collect();
    assert!(!cases.is_empty());
    cases
}

/// One recorded argument read.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Argument {
    /// The command token the argument follows.
    command: String,
    /// The submitted text.
    text: String,
}

/// What the reader returned: whether it returned anything at all, and what.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parsed {
    /// Whether a path argument was present.
    present: bool,
    /// The returned bytes, which are empty for a present empty argument.
    path: Option<String>,
}

/// Compares every recorded case of `test` with the reader's result, keeping a present
/// empty argument apart from an absent one.
fn reads(test: &str, expected_cases: usize) {
    let cases: Vec<(Argument, Parsed)> = cases(test);
    assert_eq!(cases.len(), expected_cases);
    for (input, expected) in cases {
        let observed = get_path_command_argument(&input.text, &input.command);
        assert_eq!(observed.is_some(), expected.present, "{:?}", input.text);
        assert_eq!(
            observed.map(str::to_owned),
            expected.path,
            "{:?}",
            input.text
        );
    }
}

#[test]
fn transfer_path_argument_keeps_token_quotes_and_literal_bytes() {
    reads(
        "transfer_path_argument_keeps_token_quotes_and_literal_bytes",
        20,
    );
}

#[test]
fn transfer_path_argument_uses_ecmascript_whitespace() {
    reads("transfer_path_argument_uses_ecmascript_whitespace", 50);
}

#[test]
fn transfer_import_argument_preserves_quotes_apostrophes_and_command_boundary() {
    reads(
        "transfer_import_argument_preserves_quotes_apostrophes_and_command_boundary",
        5,
    );
}
