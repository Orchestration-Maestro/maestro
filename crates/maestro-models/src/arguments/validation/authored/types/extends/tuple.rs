//! Extends check for sequences with declared positions.

use std::rc::Rc;

use super::extends_left::extends_left;
use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// A tuple extends a tuple of the same length whose positions it all extends.
pub(crate) fn extends_tuple<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match right.kind() {
        Some(SchemaKind::Tuple) => {
            let (Some(left), Some(right)) = (tuple_items(left), tuple_items(right)) else {
                return false;
            };
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| extends_left(left, right))
        }
        Some(SchemaKind::Array) => {
            let Some(left) = tuple_items(left) else {
                return false;
            };
            let Some(item) = super::array::item(right) else {
                return false;
            };
            left.iter().all(|member| extends_left(member, &item))
        }
        _ => extends_right(left, right),
    }
}

/// The positions a tuple node holds, composed or declared.
pub(crate) fn tuple_items<'n, 'a>(
    node: &'n Rc<Node<'a>>,
) -> Option<std::borrow::Cow<'n, [Rc<Node<'a>>]>> {
    match &**node {
        Node::Tuple(parts, _) => Some(std::borrow::Cow::Borrowed(parts)),
        Node::Leaf(view) if view.kind == Some(&crate::SchemaKind::Tuple) => {
            Some(std::borrow::Cow::Owned(
                view.children("items")?
                    .into_iter()
                    .map(|v| Rc::new(Node::Leaf(v)))
                    .collect(),
            ))
        }
        _ => None,
    }
}
