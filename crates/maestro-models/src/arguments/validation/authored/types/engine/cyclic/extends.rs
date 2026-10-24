//! Cyclic node spelling for structural comparison.

use std::rc::Rc;

use serde_json::Value;

use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{
    Converter, Member, Node, ObjectNode, View,
};

/// Spell a cyclic node's definition with every reference replaced by a node accepting
/// anything, so a recursive definition can be compared without recursing again.
pub(crate) fn cyclic_extends<'a>(converter: &Converter<'a>, view: View<'a>) -> Rc<Node<'a>> {
    let declaration = view
        .raw
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|name| view.raw.get("$defs").and_then(Value::as_object)?.get(name));
    match declaration {
        Some(declaration) => from_type_any(converter, converter.view(declaration)),
        None => Rc::new(Node::Unknown),
    }
}

/// Spell one node with every reference replaced by a node accepting anything.
fn from_type_any<'a>(converter: &Converter<'a>, view: View<'a>) -> Rc<Node<'a>> {
    let spelled = |keyword| -> Option<Vec<Rc<Node<'a>>>> {
        let children = converter.children(view, keyword)?;
        Some(
            children
                .into_iter()
                .map(|child| from_type_any(converter, child))
                .collect(),
        )
    };
    let node = match view.kind {
        Some(SchemaKind::Ref) => Some(Node::Any),
        Some(SchemaKind::Array) => converter
            .keyword(view, "items")
            .map(|item| Node::Array(from_type_any(converter, item), None)),
        Some(SchemaKind::Object) => {
            Member::declared(converter, view, |child| from_type_any(converter, child)).map(
                |properties| {
                    Node::Object(ObjectNode {
                        properties,
                        source: None,
                    })
                },
            )
        }
        Some(SchemaKind::Tuple) => spelled("items").map(|items| Node::Tuple(items, None)),
        Some(SchemaKind::Union) => spelled("anyOf").map(|members| Node::Union(members, None)),
        Some(SchemaKind::Intersect) => {
            spelled("allOf").map(|members| Node::Intersect(members, None))
        }
        _ => None,
    };
    Rc::new(node.unwrap_or(Node::Leaf(view)))
}
