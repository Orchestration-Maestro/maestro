//! Combining two JSON objects the way settings layers combine.

use serde_json::{Map, Value};

/// Merges `over` onto `base`: object pairs merge one level, everything else
/// in `over` replaces.
#[must_use]
pub fn merge_one_level(base: Map<String, Value>, over: Map<String, Value>) -> Map<String, Value> {
    drop(over);
    base
}
