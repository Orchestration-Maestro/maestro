//! Conversion through the definitions a cyclic node makes visible.

use serde_json::Value;

use super::from_type::from_type;
use crate::parameters::validation::authored::context::Context;
use crate::parameters::validation::authored::schema_view::{Node, View};

/// Convert through the cyclic node's reference with its definitions visible for that step.
pub(crate) fn from_cyclic<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let name = view.raw.get("$ref").and_then(Value::as_str)?;
    converter.within(view, |converter| {
        let target = converter.definition(name)?;
        from_type(converter, &Node::Leaf(target), value)
    })
}
