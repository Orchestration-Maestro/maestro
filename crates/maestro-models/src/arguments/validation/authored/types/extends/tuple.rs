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
            let (Some(left), Some(right)) = (left.parts(converter), right.parts(converter)) else {
                return false;
            };
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| extends_left(converter, left, right))
        }
        Some(SchemaKind::Array) => {
            let Some(left) = left.parts(converter) else {
                return false;
            };
            let Some(item) = super::array::item(converter, right) else {
                return false;
            };
            left.iter()
                .all(|member| extends_left(converter, member, &item))
        }
        _ => extends_right(converter, left, right),
    }
}
