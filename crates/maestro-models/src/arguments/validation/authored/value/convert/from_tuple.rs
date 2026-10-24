//! Conversion through the declared positions of a sequence.

use std::rc::Rc;

use serde_json::Value;

use super::convert_member;
use crate::arguments::validation::authored::schema_view::{Converter, Node, View};

/// Convert the leading positions of a sequence through their positional nodes.
pub(crate) fn from_tuple<'a>(
    converter: &mut Converter<'a>,
    items: &[Rc<Node<'a>>],
    value: &mut Value,
) -> Option<Value> {
    let values = value.as_array_mut()?;
    for (member, item) in values.iter_mut().zip(items) {
        convert_member(converter, item, member);
    }
    None
}

/// Convert an authored tuple's positions through the nodes they declare.
pub(crate) fn from_tuple_view<'a>(
    converter: &mut Converter<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let items = view.raw.get("items").and_then(Value::as_array)?;
    let nodes: Vec<Rc<Node<'a>>> = items
        .iter()
        .map(|item| Rc::new(Node::Leaf(converter.child(view, item))))
        .collect();
    from_tuple(converter, &nodes, value)
}
