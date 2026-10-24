//! Conversion through the single declared item kind of a sequence.

use serde_json::Value;

use super::{convert_member, from_type::from_type, r#try::try_array};
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, View};
use crate::arguments::validation::coercion;

/// Convert a supplied value through the item node of an authored sequence.
///
/// The returned sequence is a new one, as the reference returns it; nested container
/// mutations reach the supplied sequence's own members.
pub(crate) fn from_array<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    view.raw.get("items").filter(|items| items.is_object())?;
    let Value::Array(source) = value else {
        let mut members = try_array::try_array(value);
        coercion::sequence(view.raw, &mut members, |member, item| {
            let item = Node::Leaf(converter.document.view(item));
            convert_member(converter, &item, member);
        });
        return Some(Value::Array(members));
    };
    let mut members = Vec::with_capacity(source.len());
    coercion::sequence(view.raw, source, |member, item| {
        let item = Node::Leaf(converter.document.view(item));
        members.push(from_type(converter, &item, member).unwrap_or_else(|| member.clone()));
    });
    Some(Value::Array(members))
}
