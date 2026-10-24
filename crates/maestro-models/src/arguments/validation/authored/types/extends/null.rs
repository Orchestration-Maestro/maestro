//! Extends check for a left null node.

use std::rc::Rc;

use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// A null node extends a null right node and asks every other right node.
pub(crate) fn extends_null<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    if right.kind() == Some(SchemaKind::Null) {
        return true;
    }
    extends_right(left, right)
}
