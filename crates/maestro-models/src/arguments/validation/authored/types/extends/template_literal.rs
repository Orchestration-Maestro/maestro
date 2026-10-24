//! Extends check for template literal nodes.

use std::rc::Rc;

use super::extends_left::extends_left;
use crate::arguments::validation::authored::schema_view::Node;
use crate::arguments::validation::authored::types::engine::template_literal::decode::decode;

/// A template literal node extends what its decoded alternatives extend.
pub(crate) fn extends_template_literal<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match decoded(left) {
        Some(decoded) => extends_left(&decoded, right),
        None => false,
    }
}

/// A left node extends a template literal right node when it extends its decoded node.
pub(crate) fn extends_decoded<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match decoded(right) {
        Some(decoded) => extends_left(left, &decoded),
        None => false,
    }
}

/// The node a template literal pattern decodes into, when the pattern is finite.
fn decoded<'a>(node: &Rc<Node<'a>>) -> Option<Rc<Node<'a>>> {
    match &**node {
        Node::Leaf(view) => view
            .raw
            .get("pattern")
            .and_then(serde_json::Value::as_str)
            .and_then(decode),
        Node::Union(..) => Some(Rc::clone(node)),
        _ => None,
    }
}
