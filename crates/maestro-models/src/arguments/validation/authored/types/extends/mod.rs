//! Structural comparison of schema nodes.

use std::rc::Rc;

use self::extends_left::extends_left;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, Node};
use crate::arguments::validation::authored::types::engine::cyclic::extends::cyclic_extends;

pub(crate) mod any;
pub(crate) mod array;
pub(crate) mod boolean;
pub(crate) mod enumeration;
pub(crate) mod extends_left;
pub(crate) mod extends_right;
pub(crate) mod integer;
pub(crate) mod intersect;
pub(crate) mod literal;
pub(crate) mod never;
pub(crate) mod null;
pub(crate) mod number;
pub(crate) mod object;
pub(crate) mod string;
pub(crate) mod template_literal;
pub(crate) mod tuple;
pub(crate) mod union;
pub(crate) mod unknown;

/// Whether every value the left node accepts is accepted by the right node.
pub(crate) fn extends<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    let left = canonical(converter, left);
    let right = canonical(converter, right);
    extends_left(converter, &left, &right)
}

/// The node the check compares, spelling out a cyclic node's definition.
fn canonical<'a>(converter: &Converter<'a>, node: &Rc<Node<'a>>) -> Rc<Node<'a>> {
    match &**node {
        Node::Leaf(view) if view.kind == Some(&SchemaKind::Cyclic) => {
            cyclic_extends(converter, *view)
        }
        _ => Rc::clone(node),
    }
}
