//! Evaluation of intersected and alternative schema nodes.

use std::rc::Rc;

use self::{broaden as broadening, distribute as distributing};
use crate::arguments::validation::authored::schema_view::Node;

pub(crate) mod broaden;
pub(crate) mod compare;
pub(crate) mod composite;
pub(crate) mod distribute;
pub(crate) mod flatten;
pub(crate) mod narrow;

/// Evaluate one node: an intersection distributes and broadens, a union broadens.
pub(crate) fn evaluate_type<'a>(node: &Rc<Node<'a>>) -> Rc<Node<'a>> {
    match &**node {
        Node::Intersect(members, _) => evaluate_intersect(members),
        Node::Union(alternatives, _) => broadening::broaden(alternatives),
        _ => Rc::clone(node),
    }
}

/// Evaluate an intersection into the node conversion uses.
pub(crate) fn evaluate_intersect<'a>(members: &[Rc<Node<'a>>]) -> Rc<Node<'a>> {
    let distribution = distributing::distribute(members);
    broadening::broaden(&distribution)
}
