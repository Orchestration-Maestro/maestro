//! Shared strict selector queries and complete output records.
use maestro_app::ThinkingLevel;
use maestro_tui::Component;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
/// One corpus query owned by a named witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Witness name.
    test: String,
    /// Input decoded by the witness.
    input: Value,
    /// Output decoded by the witness.
    expected: Value,
    /// Complete macOS layout when its key names differ.
    expected_macos: Option<Value>,
}
/// Load strictly decoded corpus rows.
pub fn load(source: &str) -> Vec<Case> {
    serde_json::from_str(source).unwrap()
}
/// Decode a strict nonempty group after validating corpus ownership.
pub fn cases<I: DeserializeOwned, E: DeserializeOwned>(
    rows: Vec<Case>,
    owners: &[&str],
    name: &str,
) -> Vec<(I, E)> {
    let mut queries = std::collections::HashSet::new();
    for row in &rows {
        assert!(owners.contains(&row.test.as_str()), "orphan corpus group");
        assert!(
            queries.insert((row.test.clone(), row.input.to_string())),
            "duplicate corpus query"
        );
    }
    assert!(
        owners
            .iter()
            .all(|owner| rows.iter().any(|row| row.test == *owner)),
        "missing corpus group"
    );
    let result: Vec<_> = rows
        .into_iter()
        .filter(|row| row.test == name)
        .map(|row| {
            (
                serde_json::from_value(row.input).unwrap(),
                serde_json::from_value(if cfg!(target_os = "macos") {
                    row.expected_macos.unwrap_or(row.expected)
                } else {
                    row.expected
                })
                .unwrap(),
            )
        })
        .collect();
    assert!(!result.is_empty(), "missing corpus group");
    result
}
/// Complete domain inventory, in source order.
pub fn levels() -> Vec<ThinkingLevel> {
    vec![
        ThinkingLevel::Off,
        ThinkingLevel::Minimal,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Xhigh,
    ]
}
/// Deliver keys to the public input interface.
pub fn input(component: &dyn Component, keys: &[&str]) {
    for key in keys {
        component.input_handler().unwrap().handle_input(key);
    }
}
/// Empty query with no unread members.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
/// Entire output with caller notifications.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Complete rows.
    pub lines: Vec<String>,
    /// Ordered notifications.
    pub events: Vec<Vec<Value>>,
}
/// Every observable frame after input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frames {
    /// Ordered frames.
    pub frames: Vec<Frame>,
}
