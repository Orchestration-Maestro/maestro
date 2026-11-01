//! Extends check for a left boolean node.

use std::rc::Rc;

use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// A boolean node extends a boolean right node and asks every other right node.
pub(crate) fn extends_boolean<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    if right.kind() == Some(SchemaKind::Boolean) {
        return true;
    }
    extends_right(left, right)
}
