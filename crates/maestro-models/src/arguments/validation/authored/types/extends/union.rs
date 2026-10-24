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
    let Some(left_alternatives) = left.parts(converter) else {
        return false;
    };
    let right_alternatives = right
        .parts(converter)
        .unwrap_or_else(|| std::slice::from_ref(right).into());
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
    right.parts(converter).is_some_and(|alternatives| {
        alternatives
            .iter()
            .any(|alternative| extends_left(converter, left, alternative))
    })
}
