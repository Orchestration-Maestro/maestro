//! Dispatch of the extends check by the right node's kind.

use std::rc::Rc;

use super::{
    enumeration::extends_enum_values, intersect::extends_right_intersect,
    template_literal::extends_decoded, union::extends_right_union,
};
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// Perform the structural check a left handler falls through to.
///
/// A right node that accepts anything accepts the left node; an alternative set accepts it
/// when one of its alternatives does; every other right kind is asked through its own handler.
pub(crate) fn extends_right<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    match right.kind() {
        Some(SchemaKind::Any | SchemaKind::Unknown) => true,
        Some(SchemaKind::Enum) => extends_enum_values(left, right),
        Some(SchemaKind::Intersect) => extends_right_intersect(left, right),
        Some(SchemaKind::TemplateLiteral) => extends_decoded(left, right),
        Some(SchemaKind::Union) => extends_right_union(left, right),
        _ => false,
    }
}
