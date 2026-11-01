//! Extends check for objects with declared members.

use std::rc::Rc;

use super::extends_left::extends_left;
use super::extends_right::extends_right;
use crate::SchemaKind;
use crate::arguments::validation::authored::schema_view::{Members, Node};

/// An object extends an object whose every member it declares and extends; it may leave out
/// a member the other object marks optional.
pub(crate) fn extends_object<'a>(left: &Rc<Node<'a>>, right: &Rc<Node<'a>>) -> bool {
    if right.kind() != Some(SchemaKind::Object) {
        return extends_right(left, right);
    }
    let (Some(left), Some(right)) = (
        crate::arguments::validation::authored::types::object::properties(left),
        crate::arguments::validation::authored::types::object::properties(right),
    ) else {
        return false;
    };
    let left = Members::new(&left);
    right.iter().all(|right| {
        left.get(&right.name).map_or(right.optional, |left| {
            (!left.optional || right.optional) && extends_left(&left.node, &right.node)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::extends_object;
    use crate::arguments::validation::authored::context::Context;
    use crate::arguments::validation::authored::schema_view::MEMBER_LOOKUPS;
    use crate::arguments::validation::authored::types::engine::evaluate::composite::tests::operands;
    use crate::arguments::validation::authored::types::engine::instantiate::instantiate;

    /// Comparing two objects looks each right member up once in the left object.
    #[test]
    fn comparison_looks_each_right_member_up_once() {
        for width in [16, 32, 64] {
            let (schema, kinds) = operands(width);
            let document =
                crate::arguments::validation::authored::schema_view::Document::new(&schema, &kinds);
            let code = crate::arguments::validation::check::Code::default();
            let converter = Context::new(&document, &code);
            let left = instantiate(&converter, converter.document.view(&schema[0]));
            MEMBER_LOOKUPS.with(|count| count.set(0));
            assert!(extends_object(&left, &left));
            let lookups = MEMBER_LOOKUPS.with(std::cell::Cell::get);
            assert!(
                lookups <= width,
                "{lookups} lookups for {width} right members"
            );
        }
    }
}
