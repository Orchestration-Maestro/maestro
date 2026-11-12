//! Extends check for a left number node.

use std::rc::Rc;

use super::extends_right::extends_right;
use crate::authored::schema_view::Node;
use maestro_request::parameter_schema::SchemaKind;

/// A number node extends a number right node and asks every other right node.
pub(crate) fn extends_number<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    if right.kind() == Some(SchemaKind::Number) {
        return true;
    }
    extends_right(left, right)
}
