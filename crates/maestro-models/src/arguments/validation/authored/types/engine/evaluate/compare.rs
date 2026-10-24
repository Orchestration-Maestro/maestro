//! Set relation between two evaluated nodes.

use std::rc::Rc;

use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;
use crate::arguments::validation::authored::types::extends::extends;

/// Relation of one node's values to another node's values.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Relation {
    /// Both nodes accept the same values.
    Equal,
    /// Neither node accepts a value the other accepts.
    Disjoint,
    /// Every value of the left node is accepted by the right node.
    LeftInside,
    /// Every value of the right node is accepted by the left node.
    RightInside,
}

/// Compare the supplied nodes and determine their set relation.
pub(crate) fn compare<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> Relation {
    let left_unknown = left.kind() == Some(SchemaKind::Unknown);
    let left_extends = !left_unknown && extends(left, right);
    let right_extends = if left_unknown {
        true
    } else {
        extends(right, left)
    };
    match (left_extends, right_extends) {
        (true, true) => Relation::Equal,
        (true, false) => Relation::LeftInside,
        (false, true) => Relation::RightInside,
        (false, false) => Relation::Disjoint,
    }
}
