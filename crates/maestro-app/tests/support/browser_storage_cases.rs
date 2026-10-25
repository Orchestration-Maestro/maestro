//! Whole-collection observations on the controlled adapter.
use super::support::Backend;
use serde_json::{Value, json};
impl Backend {
    /// Read a complete collection as ordered key/value pairs.
    pub fn entries(&self, name: &str) -> Value {
        Value::Array(
            self.data
                .borrow()
                .get(name)
                .into_iter()
                .flat_map(|m| m.iter())
                .map(|(k, v)| json!([k, v]))
                .collect(),
        )
    }
}
