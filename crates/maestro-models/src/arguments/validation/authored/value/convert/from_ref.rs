//! Conversion through the definition a reference names.

use serde_json::Value;

use super::from_type::from_type;
use crate::arguments::validation::authored::schema_view::{Converter, Node, View};

/// Convert through the definition the supplied reference names.
pub(crate) fn from_ref<'a>(
    converter: &mut Converter<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let name = view.raw.get("$ref").and_then(Value::as_str)?;
    let target = converter.definition(view, name)?;
    from_type(converter, &Node::Leaf(target), value)
}
