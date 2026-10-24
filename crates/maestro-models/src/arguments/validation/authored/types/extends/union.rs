//! Extends check for alternative sets.

use std::rc::Rc;

use super::extends_left::extends_left;
use crate::arguments::validation::authored::schema_view::Node;

/// Every alternative of an alternative set must extend one right alternative.
pub(crate) fn extends_union<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    let Some(left_alternatives) = union_types(left) else {
        return false;
    };
    let right_alternatives =
        union_types(right).unwrap_or_else(|| std::slice::from_ref(right).into());
    left_alternatives.iter().all(|left| {
        right_alternatives
            .iter()
            .any(|right| extends_left(left, right))
    })
}

/// A left node extends an alternative set when one alternative accepts it.
pub(crate) fn extends_right_union<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    union_types(right).is_some_and(|alternatives| {
        alternatives
            .iter()
            .any(|alternative| extends_left(left, alternative))
    })
}

/// The alternatives an alternative set holds, composed or declared.
pub(crate) fn union_types<'n, 'a>(
    node: &'n Rc<Node<'a>>,
) -> Option<std::borrow::Cow<'n, [Rc<Node<'a>>]>> {
    match &**node {
        Node::Union(parts, _) => Some(std::borrow::Cow::Borrowed(parts)),
        Node::Leaf(view) if view.kind == Some(&crate::SchemaKind::Union) => {
            Some(std::borrow::Cow::Owned(
                view.children("anyOf")?
                    .into_iter()
                    .map(|v| Rc::new(Node::Leaf(v)))
                    .collect(),
            ))
        }
        _ => None,
    }
}
