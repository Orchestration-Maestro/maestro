//! Wire-form layout trees for the layout command tests.

use serde_json::{Value, json};

/// A leaf that starts the configured shell.
pub fn shell() -> Value {
    json!({"type": "pane"})
}

/// A leaf that runs a long sleep, so its pane outlives the test body.
pub fn sleeper() -> Value {
    json!({"type": "pane", "command": ["/bin/sleep", "60"]})
}

/// An even split to the right of two subtrees.
pub fn split(first: Value, second: Value) -> Value {
    let mut node = json!({"type": "split", "direction": "right", "ratio": 0.5});
    node["first"] = first;
    node["second"] = second;
    node
}

/// A tree of `leaves` leaves that always splits its first child: one level per leaf.
pub fn chain(leaves: usize, leaf: &Value) -> Value {
    (1..leaves).fold(leaf.clone(), |first, _| split(first, leaf.clone()))
}

/// A tree of `leaves` leaves whose halves differ by at most one leaf.
pub fn balanced(leaves: usize, leaf: &Value) -> Value {
    if leaves <= 1 {
        return leaf.clone();
    }
    split(
        balanced(leaves / 2, leaf),
        balanced(leaves - leaves / 2, leaf),
    )
}
