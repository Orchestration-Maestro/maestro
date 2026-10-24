//! Extends check for sequences with one item node.

use std::rc::Rc;

use super::extends_left::extends_left;
use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, Node};

/// A sequence extends a sequence with an item node its own item node extends.
pub(crate) fn extends_array<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    if right.kind() == Some(SchemaKind::Array) {
        return match (item(converter, left), item(converter, right)) {
            (Some(left), Some(right)) => extends_left(converter, &left, &right),
            _ => false,
        };
    }
    extends_right(converter, left, right)
}

/// The item node a sequence declares.
pub(crate) fn item<'a>(converter: &Converter<'a>, node: &Rc<Node<'a>>) -> Option<Rc<Node<'a>>> {
    match &**node {
        Node::Array(item, _) => Some(Rc::clone(item)),
        Node::Leaf(view) => Some(Rc::new(Node::Leaf(converter.keyword(*view, "items")?))),
        _ => None,
    }
}
