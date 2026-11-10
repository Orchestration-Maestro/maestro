//! Rendered values of a prepared line difference, projected for native tests.
use super::DiffContent;
use serde_json::{Value, json};
use std::any::Any;

/// Title, classes and the signed lines in display order.
pub(crate) fn prepared(content: &dyn Any) -> Option<Value> {
    content.downcast_ref::<DiffContent>().map(|DiffContent(diff)| {
        json!({
            "title": diff.title,
            "className": diff.class_name,
            "rows": diff.rows.iter().map(|row| json!({"sign": row.sign.to_string(), "text": row.text})).collect::<Vec<_>>(),
        })
    })
}
