//! Extends check for objects with declared members.

use std::rc::Rc;

use super::extends_left::extends_left;
use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, Node};

/// An object extends an object whose every member it declares and extends.
pub(crate) fn extends_object<'a>(
    converter: &Converter<'a>,
    left: &Rc<Node<'a>>,
    right: &Rc<Node<'a>>,
) -> bool {
    if right.kind() != Some(SchemaKind::Object) {
        return extends_right(converter, left, right);
    }
    let (Some(left), Some(right)) = (left.members(), right.members()) else {
        return false;
    };
    right.iter().all(|(name, right_member)| {
        left.iter()
            .find(|(member, _)| member == name)
            .is_some_and(|(_, left_member)| extends_left(converter, left_member, right_member))
    })
}
