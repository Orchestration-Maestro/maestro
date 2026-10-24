//! Extends check for alternative sets.

use std::rc::Rc;

use super::extends_left::extends_left;
use crate::arguments::validation::authored::schema_view::{Converter, Node};

/// Every alternative of an alternative set must extend one right alternative.
pub(crate) fn extends_union<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    let Some(left_alternatives) = alternatives(left) else {
        return false;
    };
    let right_alternatives = match alternatives(right) {
        Some(right_alternatives) => right_alternatives,
        None => vec![Rc::clone(right)],
    };
    left_alternatives.iter().all(|left| {
        right_alternatives
            .iter()
            .any(|right| extends_left(converter, left, right))
    })
}

/// A left node extends an alternative set when one alternative accepts it.
pub(crate) fn extends_right_union<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    alternatives(right).is_some_and(|alternatives| {
        alternatives
            .iter()
            .any(|alternative| extends_left(converter, left, alternative))
    })
}

/// The alternatives an alternative set declares.
fn alternatives<'a>(node: &Rc<Node<'a>>) -> Option<Vec<Rc<Node<'a>>>> {
    match &**node {
        Node::Union(alternatives) => Some(alternatives.clone()),
        Node::Leaf(view) => Some(
            view.raw
                .get("anyOf")
                .and_then(serde_json::Value::as_array)?
                .iter()
                .map(|alternative| {
                    Rc::new(Node::Leaf(
                        crate::arguments::validation::authored::schema_view::View {
                            raw: alternative,
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
