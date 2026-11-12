//! Helpers that the pane rename tests share between the integration program and the unit tests.

use serde_json::{Value, json};

/// A rename request for the first pane of the first workspace; `None` leaves the label out.
#[must_use]
pub fn rename(label: Option<&str>) -> Value {
    let mut wire = json!({"type": "pane.rename", "pane_id": "w1:p1"});
    if let Some(label) = label {
        wire["label"] = json!(label);
    }
    wire
}
