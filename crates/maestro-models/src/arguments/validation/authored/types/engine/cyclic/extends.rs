//! Cyclic node spelling for structural comparison.

use std::{borrow::Cow, rc::Rc};

use serde_json::Value;

use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, Node, ObjectNode, View};

/// Spell a cyclic node's definition with every reference replaced by a node accepting
/// anything, so a recursive definition can be compared without recursing again.
pub(crate) fn cyclic_extends<'a>(converter: &Converter<'a>, view: View<'a>) -> Rc<Node<'a>> {
    let Some(name) = view.raw.get("$ref").and_then(Value::as_str) else {
        return Rc::new(Node::Unknown);
    };
    let declaration = view
        .raw
        .get("$defs")
        .and_then(Value::as_object)
        .and_then(|definitions| definitions.get(name));
    let Some(declaration) = declaration else {
        return Rc::new(Node::Unknown);
    };
    from_type_any(converter, converter.child(view, declaration))
}

/// Spell one node with every reference replaced by a node accepting anything.
fn from_type_any<'a>(converter: &Converter<'a>, view: View<'a>) -> Rc<Node<'a>> {
    match view.kind {
        Some(SchemaKind::Ref) => Rc::new(Node::Any),
        Some(SchemaKind::Array) => match converter.keyword(view, "items") {
            Some(item) => Rc::new(Node::Array(from_type_any(converter, item))),
            None => Rc::new(Node::Leaf(view)),
        },
        Some(SchemaKind::Object) => match view.raw.get("properties").and_then(Value::as_object) {
            Some(properties) => {
                let properties: Vec<(Cow<'a, str>, Rc<Node<'a>>)> = properties
                    .iter()
                    .map(|(name, child)| {
                        (
                            Cow::Borrowed(name.as_str()),
                            from_type_any(converter, converter.child(view, child)),
                        )
                    })
                    .collect();
                Rc::new(Node::Object(ObjectNode {
                    properties,
                    source: None,
                }))
            }
            None => Rc::new(Node::Leaf(view)),
        },
        Some(SchemaKind::Tuple) => children(converter, view, "items").map_or_else(
            || Rc::new(Node::Leaf(view)),
            |children| {
                Rc::new(Node::Tuple(
                    children
                        .into_iter()
                        .map(|child| from_type_any(converter, child))
                        .collect(),
                ))
            },
        ),
        Some(SchemaKind::Union) => children(converter, view, "anyOf").map_or_else(
            || Rc::new(Node::Leaf(view)),
            |children| {
                Rc::new(Node::Union(
                    children
                        .into_iter()
                        .map(|child| from_type_any(converter, child))
                        .collect(),
                ))
            },
        ),
        Some(SchemaKind::Intersect) => children(converter, view, "allOf").map_or_else(
            || Rc::new(Node::Leaf(view)),
            |children| {
                Rc::new(Node::Intersect(
                    children
                        .into_iter()
                        .map(|child| from_type_any(converter, child))
                        .collect(),
                ))
            },
        ),
        _ => Rc::new(Node::Leaf(view)),
    }
}

/// The child nodes a keyword names, in declaration order.
fn children<'a>(converter: &Converter<'a>, view: View<'a>, keyword: &str) -> Option<Vec<View<'a>>> {
    let children = view.raw.get(keyword)?.as_array()?;
    Some(
        children
            .iter()
            .map(|child| converter.child(view, child))
            .collect(),
    )
}
