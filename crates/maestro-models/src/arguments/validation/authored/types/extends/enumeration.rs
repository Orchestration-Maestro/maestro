//! Extends check for enumerated nodes.

use std::rc::Rc;

use super::extends_left::extends_left;
use crate::arguments::validation::authored::schema_view::Node;
use crate::arguments::validation::authored::types::engine::enumeration::enum_to_union;

/// An enumerated node extends what the union of its constants extends.
pub(crate) fn extends_enum<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match enumeration(left) {
        Some(constants) => extends_left(&enum_union(constants), right),
        None => false,
    }
}

/// A left node extends an enumerated right node when it extends the union of its constants.
pub(crate) fn extends_enum_values<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match enumeration(right) {
        Some(constants) => extends_left(left, &enum_union(constants)),
        None => false,
    }
}

/// The constants an enumerated node declares.
fn enumeration<'a>(node: &Node<'a>) -> Option<&'a [serde_json::Value]> {
    match node {
        Node::Leaf(view) => view
            .raw
            .get("enum")
            .and_then(serde_json::Value::as_array)
            .map(Vec::as_slice),
        _ => None,
    }
}

/// The union node a constant list stands for.
pub(crate) fn enum_union(constants: &[serde_json::Value]) -> Rc<Node<'_>> {
    Rc::new(Node::Union(enum_to_union(constants), None))
}
