//! Extends check for a left string node.

use std::rc::Rc;

use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::Node;

/// A string node extends a string right node and asks every other right node.
pub(crate) fn extends_string<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    if right.kind() == Some(SchemaKind::String) {
        return true;
    }
    extends_right(left, right)
}
