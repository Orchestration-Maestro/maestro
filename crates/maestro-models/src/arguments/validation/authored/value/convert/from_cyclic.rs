//! Conversion through the definitions a cyclic node makes visible.

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};

/// Convert through the cyclic node's reference with its definitions visible for that step.
pub(crate) fn from_cyclic<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    let Some(name) = view.raw.get("$ref").and_then(Value::as_str) else {
        return Ok(None);
    };
    converter.within(view, |converter| {
        let Some(target) = converter.definition(name) else {
            return Ok(None);
        };
        from_type(converter, &Node::Leaf(target), value)
    })
}
