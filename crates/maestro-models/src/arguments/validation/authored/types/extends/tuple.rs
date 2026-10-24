//! Extends check for sequences with declared positions.

use std::rc::Rc;

use super::extends_left::extends_left;
use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, Node};

/// A tuple extends a tuple of the same length whose positions it all extends.
pub(crate) fn extends_tuple<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    match right.kind() {
        Some(SchemaKind::Tuple) => {
            let (Some(left), Some(right)) = (items(left), items(right)) else {
                return false;
            };
            left.len() == right.len()
                && left
                    .iter()
                    .zip(&right)
                    .all(|(left, right)| extends_left(converter, left, right))
        }
        Some(SchemaKind::Array) => {
            let Some(left) = items(left) else {
                return false;
            };
            let Some(item) = super::array::item(right) else {
                return false;
            };
            left.iter()
                .all(|member| extends_left(converter, member, &item))
        }
        _ => extends_right(converter, left, right),
    }
}

/// The position nodes a tuple declares.
fn items<'a>(node: &Rc<Node<'a>>) -> Option<Vec<Rc<Node<'a>>>> {
    match &**node {
        Node::Tuple(items) => Some(items.clone()),
        Node::Leaf(view) => Some(
            view.raw
                .get("items")
                .and_then(serde_json::Value::as_array)?
                .iter()
                .map(|item| {
                    Rc::new(Node::Leaf(
                        crate::arguments::validation::authored::schema_view::View {
                            raw: item,
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
