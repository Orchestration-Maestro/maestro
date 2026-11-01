//! Extends check for intersected nodes.

use std::rc::Rc;

use super::extends_left::extends_left;
use crate::arguments::validation::authored::schema_view::Node;
use crate::arguments::validation::authored::types::engine::evaluate::evaluate_intersect;

/// An intersected node extends what its evaluated members extend.
pub(crate) fn extends_intersect<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match intersect_types(left) {
        Some(members) => extends_left(&evaluate_intersect(&members), right),
        None => false,
    }
}

/// A left node extends an intersected right node when it extends every member.
pub(crate) fn extends_right_intersect<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match intersect_types(right) {
        Some(members) => members.iter().all(|member| extends_left(left, member)),
        None => false,
    }
}

/// The members an intersected node holds, composed or declared.
pub(crate) fn intersect_types<'n, 'a>(
    node: &'n Rc<Node<'a>>,
) -> Option<std::borrow::Cow<'n, [Rc<Node<'a>>]>> {
    match &**node {
        Node::Intersect(parts, _) => Some(std::borrow::Cow::Borrowed(parts)),
        Node::Leaf(view) if view.kind == Some(&crate::SchemaKind::Intersect) => {
            Some(std::borrow::Cow::Owned(
                view.children("allOf")?
                    .into_iter()
                    .map(|v| Rc::new(Node::Leaf(v)))
                    .collect(),
            ))
        }
        _ => None,
    }
}
