//! Extends check for a left literal node.

use std::rc::Rc;

use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// A literal node extends the matching literal or the matching scalar kind.
pub(crate) fn extends_literal<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    let Some(constant) = constant(left) else {
        return false;
    };
    let matching = match constant {
        serde_json::Value::Bool(_) => SchemaKind::Boolean,
        serde_json::Value::Number(_) => SchemaKind::Number,
        serde_json::Value::String(_) => SchemaKind::String,
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            return extends_right(left, right);
        }
    };
    match right.kind() {
        Some(SchemaKind::Literal) => literal_value(right).is_some_and(|declared| {
            crate::arguments::validation::scalars::equal(declared, constant)
        }),
        Some(kind) if kind == matching => true,
        _ => extends_right(left, right),
    }
}

/// The constant the supplied literal node declares.
fn constant<'a>(node: &'a Rc<Node<'a>>) -> Option<&'a serde_json::Value> {
    match &**node {
        Node::Constant(constant) => Some(constant.as_ref()),
        Node::Leaf(view) => view.raw.get("const"),
        _ => None,
    }
}

/// The constant the supplied right literal node declares.
fn literal_value<'a>(node: &'a Rc<Node<'a>>) -> Option<&'a serde_json::Value> {
    constant(node)
}
