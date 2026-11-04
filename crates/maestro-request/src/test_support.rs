//! Controlled-test records of the request owner.

use serde_json::{Value, json};

/// A usage record whose counts and costs are all zero, as stored in a message.
#[must_use]
pub fn zero_usage() -> Value {
    json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}})
}
