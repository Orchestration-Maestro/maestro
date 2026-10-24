//! Narrowing of two members that share no named structure.

use std::rc::Rc;

use super::compare::{Relation, compare};
use crate::arguments::validation::authored::schema_view::{Converter, Node};

/// Narrow the supplied nodes to the one that accepts fewer values.
pub(crate) fn narrow<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> Rc<Node<'a>> {
    match compare(converter, left, right) {
        Relation::LeftInside => Rc::clone(left),
        Relation::RightInside | Relation::Equal => Rc::clone(right),
        Relation::Disjoint => Rc::new(Node::Never),
    }
}
