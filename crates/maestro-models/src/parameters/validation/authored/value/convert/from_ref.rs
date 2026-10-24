//! Conversion through the definition a reference names.

use serde_json::Value;

use super::from_type::from_type;
use crate::parameters::validation::authored::context::Context;
use crate::parameters::validation::authored::schema_view::{Node, View};

/// Convert through the definition the supplied reference names in the current frames.
pub(crate) fn from_ref<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let name = view.raw.get("$ref").and_then(Value::as_str)?;
    let target = converter.definition(name)?;
    from_type(converter, &Node::Leaf(target), value)
}
