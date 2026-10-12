//! Recorded simple-key selection cases shared by the six existing protocols.

use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

use crate::chat::TestResult;
use crate::environment_rows::check_by_environment;

/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Test that owns the case.
    test: String,
    /// Case input.
    pub input: Input,
    /// Recorded result.
    pub expected: Expected,
}

/// Case input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Protocol the case is sent to.
    pub api: String,
    /// Provider of the model, which selects the environment variable.
    pub provider: String,
    /// Supplied options.
    pub options: Options,
    /// Variables supplied to the process that evaluates the case; see the child runner.
    env: BTreeMap<String, String>,
}

/// Supplied options.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Options {
    /// Supplied key.
    pub api_key: Option<String>,
}

/// Recorded result: a key or a failure message.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Key the final request must authenticate with.
    pub key: Option<String>,
    /// Message of the setup failure.
    pub error: Option<String>,
}

/// An account token whose payload names an account, as the response-session protocol requires.
///
/// The token is assembled here so no token-shaped literal is committed.
fn token(suffix: &str) -> String {
    let payload = URL_SAFE_NO_PAD
        .encode(r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"controlled"}}"#);
    format!("a.{payload}.{suffix}")
}

/// Replace `{account-token:<suffix>}` markers with assembled tokens.
fn materialize(text: &str) -> String {
    let mut text = text.to_owned();
    for suffix in ["b", "c"] {
        text = text.replace(&format!("{{account-token:{suffix}}}"), &token(suffix));
    }
    text
}

/// Every case, checked for ownership and duplicate inputs, with tokens assembled.
fn all_rows(owners: &[&str]) -> TestResult<Vec<Row>> {
    let text = materialize(include_str!("../fixtures/simple_key_selection.json"));
    let rows: Vec<Row> = serde_json::from_str(&text)?;
    let mut inputs = BTreeSet::new();
    for row in &rows {
        assert!(owners.contains(&row.test.as_str()), "unassigned row");
        assert!(
            inputs.insert(
                format!("{}{}", row.test, serde_json::to_string(&row.input.env)?)
                    + &row.input.api
                    + &row.input.provider
                    + &format!("{:?}", row.input.options.api_key)
            ),
            "duplicate query"
        );
    }
    Ok(rows)
}

/// Check every case of `test`, each in a child process holding its recorded environment.
///
/// # Errors
/// Returns the first fixture or case failure.
pub fn assert_rows(owners: &[&str], test: &str, check: impl Fn(&Row) -> TestResult) -> TestResult {
    let rows: Vec<Row> = all_rows(owners)?
        .into_iter()
        .filter(|row| row.test == test)
        .collect();
    assert!(!rows.is_empty(), "{test} has no rows");
    check_by_environment(test, &rows, |row| Some(&row.input.env), check)
}
