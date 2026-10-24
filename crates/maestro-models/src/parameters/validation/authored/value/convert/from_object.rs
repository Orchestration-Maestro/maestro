//! Conversion through the declared members of an object.

use serde_json::Value;

use super::convert_member;
use super::from_type::from_type;
use crate::parameters::validation::authored::context::Context;
use crate::parameters::validation::authored::schema_view::{Node, ObjectNode, View};
use crate::parameters::validation::coercion;

/// Convert the present members of an instantiated or composed object.
pub(crate) fn from_object<'a>(
    converter: &mut Context<'a>,
    object: &ObjectNode<'a>,
    value: &mut Value,
) -> Option<Value> {
    match object.source {
        Some(view) => from_object_view(converter, view, value),
        None => from_composed(converter, object, value),
    }
}

/// Convert a supplied value through the members an authored object declares.
///
/// Only present members convert; a member the value does not carry is never inserted.
pub(crate) fn from_object_view<'a>(
    converter: &mut Context<'a>,
    view: View<'a>,
    value: &mut Value,
) -> Option<Value> {
    let object = value.as_object_mut()?;
    coercion::members(view.raw, object, |member, child| {
        let child = Node::Leaf(converter.document.view(child));
        convert_member(converter, &child, member);
    });
    None
}

/// Convert a supplied value through the members a composed object carries.
fn from_composed<'a>(
    converter: &mut Context<'a>,
    object: &ObjectNode<'a>,
    value: &mut Value,
) -> Option<Value> {
    let members = value.as_object_mut()?;
    for member in &object.properties {
        if let Some(supplied) = members.get_mut(member.name.as_ref())
            && let Some(replacement) = from_type(converter, &member.node, supplied)
        {
            *supplied = replacement;
        }
    }
    None
}
