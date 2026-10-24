//! Extends check for intersected nodes.

use std::rc::Rc;

use super::extends_left::extends_left;
use crate::arguments::validation::authored::schema_view::{Converter, Node};
use crate::arguments::validation::authored::types::engine::evaluate::evaluate_intersect;

/// An intersected node extends what its evaluated members extend.
pub(crate) fn extends_intersect<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    match members(left) {
        Some(members) => extends_left(converter, &evaluate_intersect(converter, &members), right),
        None => false,
    }
}

/// A left node extends an intersected right node when it extends every member.
pub(crate) fn extends_right_intersect<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    match members(right) {
        Some(members) => members
            .iter()
            .all(|member| extends_left(converter, left, member)),
        None => false,
    }
}

/// The members an intersected node declares.
fn members<'a>(node: &Rc<Node<'a>>) -> Option<Vec<Rc<Node<'a>>>> {
    match &**node {
        Node::Intersect(members) => Some(members.clone()),
        Node::Leaf(view) => Some(
            view.raw
                .get("allOf")
                .and_then(serde_json::Value::as_array)?
                .iter()
                .map(|member| {
                    Rc::new(Node::Leaf(
                        crate::arguments::validation::authored::schema_view::View {
                            raw: member,
                            kind: None,
                            visible: view.visible,
                        },
                    ))
                })
                .collect(),
        ),
        _ => None,
    }
}
