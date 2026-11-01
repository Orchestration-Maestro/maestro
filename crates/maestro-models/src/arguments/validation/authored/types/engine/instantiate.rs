//! Reference substitution that keeps cyclic references in place.

use std::rc::Rc;

use super::r#ref::instantiate::ref_instantiate;
use crate::SchemaKind;
use crate::arguments::validation::authored::context::Context;
use crate::arguments::validation::authored::schema_view::{Node, ObjectNode, View};

/// Instantiate one authored node: substitute the references that cannot recurse.
///
/// Every stage below an intersection sees substituted definitions; a reference that would
/// recurse keeps its authored node. A composed node keeps the node it was instantiated from,
/// so its options and its members' optionality stay the author's.
pub(crate) fn instantiate<'a>(converter: &Context<'a>, view: View<'a>) -> Rc<Node<'a>> {
    let instantiated = |keyword| -> Vec<Rc<Node<'a>>> {
        view.children(keyword)
            .unwrap_or_default()
            .into_iter()
            .map(|child| instantiate(converter, child))
            .collect()
    };
    Rc::new(match view.kind {
        Some(SchemaKind::Ref) => return ref_instantiate(converter, view),
        Some(SchemaKind::Array) => match view.keyword("items") {
            Some(item) => Node::Array(instantiate(converter, item), Some(view)),
            None => Node::Leaf(view),
        },
        Some(SchemaKind::Tuple) => Node::Tuple(instantiated("items"), Some(view)),
        Some(SchemaKind::Object) => Node::Object(ObjectNode {
            properties: crate::arguments::validation::authored::types::object::declared(
                view,
                |child| instantiate(converter, child),
            )
            .unwrap_or_default(),
            source: Some(view),
        }),
        Some(SchemaKind::Union) => Node::Union(instantiated("anyOf"), Some(view)),
        Some(SchemaKind::Intersect) => Node::Intersect(instantiated("allOf"), Some(view)),
        _ => Node::Leaf(view),
    })
}
