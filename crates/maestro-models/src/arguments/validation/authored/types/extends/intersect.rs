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
    match left.parts(converter) {
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
    match right.parts(converter) {
        Some(members) => members
            .iter()
            .all(|member| extends_left(converter, left, member)),
        None => false,
    }
}
