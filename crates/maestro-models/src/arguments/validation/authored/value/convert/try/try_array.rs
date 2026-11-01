//! Wrapping of a supplied value into a sequence.

/// The members the reference sequence attempt supplies.
///
/// A supplied sequence keeps its members; every other value is the single member.
pub(crate) fn try_array(value: &serde_json::Value) -> Vec<serde_json::Value> {
    match value {
        serde_json::Value::Array(members) => members.clone(),
        other => vec![other.clone()],
    }
}
