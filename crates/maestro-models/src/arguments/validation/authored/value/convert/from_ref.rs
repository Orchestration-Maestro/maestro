//! Conversion through the definition a reference names.

use crate::DiagnosticErrorInfo;
use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};

/// Convert through the definition the supplied reference names in the current frames.
pub(crate) fn from_ref<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    let Some(name) = view.raw.get("$ref").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(target) = converter.definition(name) else {
        return Ok(None);
    };
    from_type(converter, &Node::Leaf(target), value)
}
