//! Evaluation of intersected and alternative schema nodes.

use std::rc::Rc;

use self::{broaden as broadening, distribute as distributing};
use crate::arguments::validation::authored::schema_view::{Converter, Node};

pub(crate) mod broaden;
pub(crate) mod compare;
pub(crate) mod composite;
pub(crate) mod distribute;
pub(crate) mod flatten;
pub(crate) mod narrow;

/// Evaluate one node: an intersection distributes and broadens, a union broadens.
pub(crate) fn evaluate_type<'a>(converter: &Converter<'a>, node: &Rc<Node<'a>>) -> Rc<Node<'a>> {
    match &**node {
        Node::Intersect(members, _) => evaluate_intersect(converter, members),
        Node::Union(alternatives, _) => broadening::broaden(converter, alternatives),
        _ => Rc::clone(node),
    }
}

/// Evaluate an intersection into the node conversion uses.
pub(crate) fn evaluate_intersect<'a>(
    converter: &Converter<'a>,
    members: &[Rc<Node<'a>>],
) -> Rc<Node<'a>> {
    let distribution = distributing::distribute(converter, members);
    broadening::broaden(converter, &distribution)
}
