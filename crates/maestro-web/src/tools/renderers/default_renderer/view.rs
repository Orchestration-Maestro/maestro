//! Rendered values of the prepared default card, projected for native tests.
use super::DefaultContent;
use serde_json::{Value, json};
use std::any::Any;

/// Header status with the formatted input and raw output listings.
pub(in crate::tools) fn prepared(content: &dyn Any) -> Option<Value> {
    content.downcast_ref::<DefaultContent>().map(|content| {
        json!({
            "state": format!("{:?}", content.state),
            "input": content.params,
            "output": content.output,
        })
    })
}
