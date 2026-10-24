//! Extends check for objects with declared members.

use std::rc::Rc;

use super::extends_left::extends_left;
use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Converter, Members, Node};

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
    let left = Members::new(&left);
    right.iter().all(|right| {
        left.get(&right.name)
            .is_some_and(|left| extends_left(converter, &left.node, &right.node))
    })
}

#[cfg(test)]
mod tests {
    use super::extends_object;
    use crate::arguments::validation::authored::schema_view::{Converter, MEMBER_LOOKUPS};
    use crate::arguments::validation::authored::types::engine::evaluate::composite::tests::operands;
    use crate::arguments::validation::authored::types::engine::instantiate::instantiate;

    /// Comparing two objects looks each right member up once in the left object.
    #[test]
    fn comparison_looks_each_right_member_up_once() {
        for width in [16, 32, 64] {
            let (schema, kinds) = operands(width);
            let converter = Converter::new(&schema, &kinds);
            let left = instantiate(&converter, converter.view(&schema[0]));
            MEMBER_LOOKUPS.with(|count| count.set(0));
            assert!(extends_object(&converter, &left, &left));
            let lookups = MEMBER_LOOKUPS.with(std::cell::Cell::get);
            assert!(
                lookups <= width,
                "{lookups} lookups for {width} right members"
            );
        }
    }
}
