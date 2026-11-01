//! Extends check for a left integer node.

use std::rc::Rc;

use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// An integer node extends an integer or number right node and asks every other right node.
pub(crate) fn extends_integer<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    if matches!(right.kind(), Some(SchemaKind::Integer | SchemaKind::Number)) {
        return true;
    }
    extends_right(left, right)
}
