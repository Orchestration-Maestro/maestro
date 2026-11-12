//! Helpers that the pane metadata tests share between the integration program and the unit tests.

use super::api::pane_record;
use crate::state::App;
use serde_json::{Value, json};
use std::collections::HashMap;

/// A report for the first pane from `source` that carries exactly `fields`.
///
/// # Errors
/// Returns the fields that are not an object.
pub fn report(source: &str, fields: Value) -> Result<Value, String> {
    let mut wire = json!({"type": "pane.report_metadata", "pane_id": "w1:p1", "source": source});
    merge(&mut wire, fields)?;
    Ok(wire)
}

/// A title report from `user:metadata.test-1` for the first pane, with `fields` merged over it.
///
/// # Errors
/// Returns the fields that are not an object.
pub fn titled(fields: Value) -> Result<Value, String> {
    let mut wire = report("user:metadata.test-1", json!({"title": "activity"}))?;
    merge(&mut wire, fields)?;
    Ok(wire)
}

/// Adds the members of the object `fields` to the object `wire`.
fn merge(wire: &mut Value, fields: Value) -> Result<(), String> {
    match (wire.as_object_mut(), fields) {
        (Some(wire), Value::Object(fields)) => {
            wire.extend(fields);
            Ok(())
        }
        _ => Err("report fields are an object".into()),
    }
}

/// The tokens the first pane holds.
///
/// # Errors
/// Returns the answer that was not a pane record.
pub fn tokens(app: &mut App) -> Result<HashMap<String, String>, String> {
    Ok(pane_record(app, "w1:p1")?.tokens)
}
