//! Shared fixture operations for tests that live in different packages.
//!
//! A record, its selection from recorded rows and a wire-literal round trip are written
//! once here, so two packages observing the same contract assert against one implementation.
//! The single-colour tool-output theme is a macro: its construction stays at the caller,
//! which supplies its own theme crate.

use serde::de::DeserializeOwned;

/// One recorded fixture row and the public behavior that owns it.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case<I> {
    /// Public behavior group that owns this row.
    pub test: String,
    /// Operands supplied by the caller.
    pub input: I,
    /// Complete observable outcome.
    pub expected: serde_json::Value,
}

/// Decode every row of `source` and keep those owned by `test`, in source order.
///
/// The whole array is decoded before any row is selected, so a malformed row this test
/// does not own still fails. Every row with exactly this test name is retained.
///
/// # Errors
/// Returns the row decode failure, or `empty fixture group: {test}` when no row is owned
/// by this test.
pub fn cases<I: DeserializeOwned>(
    source: &str,
    test: &str,
) -> Result<Vec<Case<I>>, Box<dyn std::error::Error>> {
    let rows: Vec<Case<I>> = serde_json::from_str(source)?;
    let rows: Vec<_> = rows.into_iter().filter(|row| row.test == test).collect();
    if rows.is_empty() {
        return Err(format!("empty fixture group: {test}").into());
    }
    Ok(rows)
}

/// Report whether every supplied literal decodes and re-encodes to the same encoded value.
///
/// An empty input matches. Every literal is decoded and re-encoded, so one that differs
/// makes the result `false` rather than stopping the walk.
///
/// # Errors
/// Returns the decode or encode failure of the supplied record.
pub fn literals_round_trip<T: DeserializeOwned + serde::Serialize>(
    literals: &[&str],
) -> serde_json::Result<bool> {
    let mut matched = true;
    for literal in literals {
        let encoded = serde_json::Value::from(*literal);
        let record: T = serde_json::from_value(encoded.clone())?;
        matched &= serde_json::to_value(record)? == encoded;
    }
    Ok(matched)
}

/// Build a theme whose only colour is `toolOutput` at palette `$index`.
///
/// The operand is evaluated once, at the caller's single construction, and the theme
/// carries no backgrounds in `Color256` with default options.
#[macro_export]
macro_rules! tool_output_theme {
    ($index:expr) => {
        maestro_theme::Theme::new(
            [(
                maestro_theme::ThemeColor::ToolOutput,
                maestro_theme::ColorValue::Index($index),
            )],
            [],
            maestro_theme::ColorMode::Color256,
            maestro_theme::ThemeOptions::default(),
        )
    };
}
