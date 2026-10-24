//! Reference substitution that keeps cyclic references in place.

use std::{borrow::Cow, rc::Rc};

use serde_json::Value;

use super::{cyclic::check::cyclic_check, r#ref::instantiate::ref_instantiate};
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, Node, ObjectNode, View};

/// Instantiate one authored node: substitute the references that cannot recurse.
///
/// Every stage below an intersection sees substituted definitions; a reference that would
/// recurse keeps its authored node and its visible definitions.
pub(crate) fn instantiate<'a>(converter: &Converter<'a>, view: View<'a>) -> Rc<Node<'a>> {
    match view.kind {
        Some(SchemaKind::Ref) => ref_instantiate(converter, view, &mut Vec::new()),
        Some(SchemaKind::Array) => match converter.keyword(view, "items") {
            Some(item) => Rc::new(Node::Array(instantiate(converter, item))),
            None => Rc::new(Node::Leaf(view)),
        },
        Some(SchemaKind::Tuple) => Rc::new(Node::Tuple(
            items(converter, view)
                .into_iter()
                .map(|item| instantiate(converter, item))
                .collect(),
        )),
        Some(SchemaKind::Object) => Rc::new(Node::Object(ObjectNode {
            properties: properties(converter, view),
            source: Some(view),
        })),
        Some(SchemaKind::Union) => Rc::new(Node::Union(
            alternatives(converter, view)
                .into_iter()
                .map(|alternative| instantiate(converter, alternative))
                .collect(),
        )),
        Some(SchemaKind::Intersect) => Rc::new(Node::Intersect(
            members(converter, view)
                .into_iter()
                .map(|member| instantiate(converter, member))
                .collect(),
        )),
        _ => Rc::new(Node::Leaf(view)),
    }
}

/// Substitute a reference unless its definition recurses.
pub(crate) fn instantiate_ref<'a>(
    converter: &Converter<'a>,
    view: View<'a>,
    name: &'a str,
) -> Rc<Node<'a>> {
    let Some(target) = converter.definition(view, name) else {
        return Rc::new(Node::Leaf(view));
    };
    let mut stack = vec![name];
    if cyclic_check(converter, &mut stack, target) {
        return Rc::new(Node::Leaf(view));
    }
    instantiate(converter, target)
}

/// The declared members of an object node, in declaration order.
pub(crate) fn properties<'a>(
    converter: &Converter<'a>,
    view: View<'a>,
) -> Vec<(Cow<'a, str>, Rc<Node<'a>>)> {
    let Some(properties) = view.raw.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    properties
        .iter()
        .map(|(name, child)| {
            (
                Cow::Borrowed(name.as_str()),
                instantiate(converter, converter.child(view, child)),
            )
        })
        .collect()
}

/// The declared positions of a tuple node, in declaration order.
pub(crate) fn items<'a>(converter: &Converter<'a>, view: View<'a>) -> Vec<View<'a>> {
    let Some(items) = view.raw.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| converter.child(view, item))
        .collect()
}

/// The declared alternatives of a union node, in declaration order.
fn alternatives<'a>(converter: &Converter<'a>, view: View<'a>) -> Vec<View<'a>> {
    let Some(alternatives) = view.raw.get("anyOf").and_then(Value::as_array) else {
        return Vec::new();
    };
    alternatives
        .iter()
        .map(|alternative| converter.child(view, alternative))
        .collect()
}

/// The declared members of an intersection node, in declaration order.
fn members<'a>(converter: &Converter<'a>, view: View<'a>) -> Vec<View<'a>> {
    let Some(members) = view.raw.get("allOf").and_then(Value::as_array) else {
        return Vec::new();
    };
    members
        .iter()
        .map(|member| converter.child(view, member))
        .collect()
}
