//! Flattening of one alternative level.

use std::rc::Rc;

use crate::arguments::validation::authored::schema_view::Node;

/// Replace every alternative set by the alternatives it holds.
pub(crate) fn flatten<'a>(types: &[Rc<Node<'a>>]) -> Vec<Rc<Node<'a>>> {
    let mut result = Vec::new();
    for node in types {
        match &**node {
            Node::Union(alternatives, _) => result.extend(alternatives.iter().map(Rc::clone)),
            _ => result.push(Rc::clone(node)),
        }
    }
    result
}
