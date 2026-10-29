//! Combining two JSON objects the way settings layers combine.

use serde_json::{Map, Value};

/// Merges `over` onto `base`: object pairs merge one level, everything else
/// in `over` replaces.
#[must_use]
pub fn merge_one_level(
    mut base: Map<String, Value>,
    over: Map<String, Value>,
) -> Map<String, Value> {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Object(inner)), Value::Object(outer)) => inner.extend(outer),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
    base
}
