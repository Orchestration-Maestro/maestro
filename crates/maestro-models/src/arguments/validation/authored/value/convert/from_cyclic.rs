//! Conversion through the definitions a cyclic node makes visible.

use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::schema_view::{Converter, Node, View};

/// Make the cyclic node's definitions visible, then convert through its reference.
pub(crate) fn from_cyclic<'a>(
    converter: &mut Converter<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let name = view.raw.get("$ref").and_then(Value::as_str)?;
    converter.enter(view);
    let target = converter.visible_definition(name)?;
    from_type(converter, &Node::Leaf(target), value)
}
