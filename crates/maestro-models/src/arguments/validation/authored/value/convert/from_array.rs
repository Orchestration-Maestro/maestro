//! Conversion through the single declared item kind of a sequence.

use serde_json::Value;

use super::{convert_member, from_type::from_type, r#try::try_array};
use crate::arguments::validation::authored::schema_view::{Converter, Node, View};

/// Convert a supplied value through one instantiated item node.
///
/// The returned sequence is a new one, as the reference returns it; nested container
/// mutations reach the supplied sequence's own members.
pub(crate) fn from_array<'a>(
    converter: &mut Converter<'a>,
    item: &Node<'a>,
    value: &mut Value,
) -> Value {
    let Value::Array(source) = value else {
        let mut members = try_array::try_array(value);
        for member in &mut members {
            convert_member(converter, item, member);
        }
        return Value::Array(members);
    };
    let mut members = Vec::with_capacity(source.len());
    for member in source.iter_mut() {
        members.push(from_type(converter, item, member).unwrap_or_else(|| member.clone()));
    }
    Value::Array(members)
}

/// Convert a supplied value through the item node of an authored sequence.
pub(crate) fn from_array_view<'a>(
    converter: &mut Converter<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let item = converter.keyword(view, "items")?;
    Some(from_array(converter, &Node::Leaf(item), value))
}
